use std::mem;
use std::path::PathBuf;
use hashbrown::{HashMap, HashSet};
use nalgebra_glm::{TMat4, TVec3, TVec4, vec3, vec4};
use serde::{Deserialize, Serialize};
use crate::exports::light_components::{FindLightType, LightInformation};
use crate::exports::load_models::MaxNumLights;
use crate::exports::movement_components::Position;
use crate::exports::rendering::{DrawBuilderSystem, DrawParam, LevelOfView};
use crate::flows::render_flow::ModelRenderingInformation;
use crate::flows::shadow_flow;
use crate::models::model_definitions::{MeshGeometry, ModelId};
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::render_components::frame_buffer::{BindingTarget, FBO};
use crate::render_components::mapped_buffer::BufferWriteInfo;
use crate::render_components::texture_array::{TextureProperties, TextureUploadResult};
use crate::render_system::helper_constructs::NO_SUITABLE_TEXTURE_STORAGE_INDEX;
use crate::render_system::render_pass_resources::{RenderPassResources, UniformBufferInformation};
use crate::render_system::system_information::DrawPreparationParameters;
use crate::world::bounding_box_tree_v2::UniqueWorldSectionId;

/// ************* Helper Aliases *****************

pub type StartBufferChangedBytes = isize;
pub type NumberBytesChanged = isize;

pub type InstancedLayoutWriteFunction = fn(u32, &ECS, &mut Vec<u8>, EntityId);
pub type ModelUpdateFunction = fn(layout_index: u32, model_geometry: &MeshGeometry, buffer_write_destination: BufferWriteInfo, buffer_offset_bytes: isize) -> isize;
pub type AnyLightSourceVisible = bool;

/// Passed into uniform update function to query value of uniform entities
pub struct UniformECS<'a>
{
    pub uniform_entities: &'a HashMap<String, EntityId>,
    pub ecs: &'a ECS,
}

/// Defines the level of views to use for a render system when dealing with types of models
pub struct LevelOfViews
{
    pub default: Vec<LevelOfView>,
    pub custom: HashMap<ModelId, Vec<LevelOfView>>,
}

pub struct ModelNameLookupResult
{
    pub model_id: ModelId,
    pub uses_texture: bool,
}

const LIT_SOURCE_STENCIL_VALUE: i32 = 0xFF;

/// ************* Main Structure and Logic ***************

/// Structure that contains that required parameters to execute a render pass
pub struct RenderSystem
{
    first_render_pass_resources: RenderPassResources,
    second_render_pass_resources: Option<RenderPassResources>,
    draw_function: fn(&mut DrawParam),
    light_source_draw_function: fn(&mut DrawParam),
    transparency_draw_function: fn(&mut DrawParam),
    pub model_rendering_information: HashMap<ModelId, ModelRenderingInformation>,
    name_model_id_lookup: HashMap<String, ModelNameLookupResult>,
    model_id_name_lookup: HashMap<ModelId, String>,
    pub level_of_views: LevelOfViews,
    draw_fn_accessible_fbo: HashMap<String, FBO>,

    upload_local_lights: bool,
    is_using_skybox: bool,
    max_num_lights: MaxNumLights,
    previous_directional_lights: HashSet<EntityId>,
    previous_point_lights: HashSet<EntityId>,
    previous_spot_lights: HashSet<EntityId>,
    no_light_source_cutoff: f32,
    default_diffuse_factor: f32,
}

/// Specifies the location of an uploaded texture, as well as any scaling of the texture coordinates
/// that need to be used when using that texture
#[derive(Copy, Clone)]
pub struct UploadedTextureLocation
{
    pub array_index: usize,
    pub index_offset: i32,
    pub scale_x: f32,
    pub scale_y: f32
}

impl RenderSystem
{
    /// Creates a render system that operates on the provided resources
    ///
    /// `first_render_pass_resources` - the resources to use during the first render pass
    /// `second_render_pass_resources` - the resources to use during the second render pass
    /// `draw_function` - the function that issues model draw commands using the created render system
    /// `level_of_views` - the default level of views for the render system
    /// `draw_fn_accessible_fbo` - FBOs that can be bound by referring to their name
    /// `upload_local_lights` - boolean stating whether to use lights and therefore shadows
    pub fn new(first_render_pass_resources: RenderPassResources, second_render_pass_resources: Option<RenderPassResources>,
               draw_function: fn(&mut DrawParam),
               light_source_draw_function: fn(&mut DrawParam),
               transparency_draw_function: fn(&mut DrawParam),
               level_of_views: Vec<LevelOfView>,
               draw_fn_accessible_fbo: HashMap<String, FBO>,
               upload_local_lights: bool,
               max_light_constraints: MaxNumLights,
               no_light_source_cutoff: f32,
               default_diffuse_factor: f32) -> RenderSystem
    {
        RenderSystem
        {
            first_render_pass_resources,
            second_render_pass_resources,
            draw_function,
            light_source_draw_function,
            transparency_draw_function,
            model_rendering_information: HashMap::default(),
            name_model_id_lookup: HashMap::new(),
            model_id_name_lookup: HashMap::default(),
            level_of_views: LevelOfViews{ default: level_of_views, custom: HashMap::default(), },
            draw_fn_accessible_fbo,
            upload_local_lights,
            is_using_skybox: false,
            max_num_lights: max_light_constraints,
            previous_directional_lights: HashSet::new(),
            previous_spot_lights: HashSet::new(),
            no_light_source_cutoff,
            previous_point_lights: HashSet::new(),
            default_diffuse_factor
        }
    }

    /// Binds the render system's VAO
    pub fn use_vao(&mut self)
    {
        self.first_render_pass_resources.vao.bind();
    }

    /// Binds the render system's shader program
    pub fn use_shader_program(&mut self)
    {
        self.first_render_pass_resources.shader_program.use_shader_program();
    }

    /// Registers a type to be used as a uniform
    pub fn register_uniform_type_ecs<'a, T: 'static + Serialize + Deserialize<'a>>(&mut self)
    {
        self.first_render_pass_resources.uniform_resources.ecs.register_type::<T>();
    }

    /// Writes a value to be stored in a uniform
    ///
    /// `uniform_entity_id` - the id and value of the uniform to write for the first render pass
    pub fn write_uniform_value<'a, T: 'static + Serialize + Deserialize<'a>>(&mut self, uniform_entity_id: EntityId, value: T)
    {
        self.first_render_pass_resources.uniform_resources.ecs.write_component::<T>(uniform_entity_id, value);
    }

    /// Creates an entity to represent a uniform with the given name
    ///
    /// `uniform_name` - the name of the uniform to be created
    pub fn add_uniform_entities<T: Into<String>>(&mut self, uniform_name: T) -> EntityId
    {
        let entity_id = self.first_render_pass_resources.uniform_resources.ecs.create_entity();
        self.first_render_pass_resources.uniform_resources.uniform_entities.insert(uniform_name.into(), entity_id);
        entity_id
    }

    /// Get the function used to update instance layouts
    pub fn get_instance_layout_update_function(&self) -> Option<InstancedLayoutWriteFunction>
    {
        self.first_render_pass_resources.vertex_shader_resource.layout_update_fn
    }

    /// Get the function used to update model layouts
    pub fn get_model_layout_update_function(&self) -> ModelUpdateFunction
    {
        self.first_render_pass_resources.vertex_shader_resource.model_update_fn
    }

    pub fn will_render_skybox(&self) -> bool
    {
        self.is_using_skybox
    }

    /// Load the supplied textures into the given cubemap. This is a blocking operation
    ///
    /// `cube_map_name` - the name of the cubemap being uploaded
    /// `texture_locations` - the locations of the cubemap textures
    pub fn load_cubemap<T: AsRef<str>>(&mut self, cube_map_name: T, texture_locations: Vec<PathBuf>)
    {
        self.is_using_skybox = true;

        self.first_render_pass_resources.fragment_shader_resource.cube_maps.get_mut(cube_map_name.as_ref()).unwrap()
            .upload_texture_sequentially(texture_locations)
            .unwrap_or_else(|err| panic!("Failed to upload cubemap: {:?}", err));
    }

    /// Binds the given cubemap to the cube map OpenGL binding point
    ///
    /// `cube_name_name` - the name of the cubemap to bind
    pub fn bind_cubemap<A: AsRef<str>>(&mut self, cube_map_name: A)
    {
        self.first_render_pass_resources.fragment_shader_resource.cube_maps.get_mut(cube_map_name.as_ref()).unwrap().bind();
    }

    /// Obtain pointers to buffers that store data for instanced layouts
    pub fn get_instanced_mapped_buffers(&mut self) -> Vec<BufferWriteInfo>
    {
        self.first_render_pass_resources.vertex_shader_resource.per_instance_buffers.iter_mut()
            .map(|x| x.wait_for_next_free_buffer(1_000_000).unwrap()).collect()
    }

    /// Tell OpenGL to flush the instanced buffers. All instanced buffers must be flushed
    ///
    /// `data_changed_range` - the ranges of [x, x + number bytes changes] to flush. Index 0 of the vector
    ///                         provided refers to the first instanced layout mapped buffer, index 1 for the
    ///                         second mapped buffer, and so on
    pub fn flush_per_instance_buffers(&mut self, data_changed_range: Vec<(StartBufferChangedBytes, NumberBytesChanged)>)
    {
        for (buffer_index, (start_byte_changed, number_bytes_changed)) in data_changed_range.into_iter().enumerate()
        {
            self.first_render_pass_resources.vertex_shader_resource.per_instance_buffers[buffer_index].mark_buffer_updates_finish(start_byte_changed, number_bytes_changed);
        }
    }

    /// Obtain pointers to buffers that store data for model layouts
    pub fn get_model_mapped_buffers(&mut self) -> Vec<BufferWriteInfo>
    {
        self.first_render_pass_resources.vertex_shader_resource.per_model_buffers.iter_mut()
            .map(|x| x.wait_for_next_free_buffer(1_000_000).unwrap()).collect()
    }

    /// Tell OpenGL to flush the model buffers. All model buffers must be flushed
    ///
    /// `data_changed_range` - the ranges of [x, x + number bytes changes] to flush. Index 0 of the vector
    ///                         provided refers to the first model mapped buffer, index 1 for the
    ///                         second mapped buffer, and so on
    pub fn flush_per_model_buffers(&mut self, data_changed_range: Vec<(StartBufferChangedBytes, NumberBytesChanged)>)
    {
        for (buffer_index, (start_byte_changed, number_bytes_changed)) in data_changed_range.into_iter().enumerate()
        {
            self.first_render_pass_resources.vertex_shader_resource.per_model_buffers[buffer_index].mark_buffer_updates_finish(start_byte_changed, number_bytes_changed);
        }
    }

    pub fn add_solid_colour_texture(&mut self, texture_colour: TVec4<u8>) -> UploadedTextureLocation
    {
        let array_index = self.first_render_pass_resources.fragment_shader_resource.texture_arrays.len() - 1;
        let solid_colour_array = self.first_render_pass_resources.fragment_shader_resource.texture_arrays.last_mut();

        if let Some(texture_array) = solid_colour_array
        {
            let colour = [texture_colour[0], texture_colour[1], texture_colour[2], texture_colour[3]];
            let index = texture_array.add_texture_solid_colour(colour);
            UploadedTextureLocation
            {
                array_index,
                index_offset: index,
                scale_x: 1.0,
                scale_y: 1.0
            }
        }
        else
        {
            UploadedTextureLocation
            {
                array_index: 0,
                index_offset: NO_SUITABLE_TEXTURE_STORAGE_INDEX,
                scale_x: 1.0,
                scale_y: 1.0
            }
        }
    }

    /// Uploads the given texture to the given render system, making it available for use when rendering
    ///
    /// `texture_location` - the location of the texture to upload
    pub fn add_texture(&mut self, texture_location: PathBuf) -> UploadedTextureLocation
    {
        if let Some(upload_info) = self.first_render_pass_resources.uploaded_textures.get(&texture_location)
        {
            return *upload_info;
        }

        let texture_properties = TextureProperties::read_image(&texture_location);

        let mut most_suitable_array_index = None;
        let mut least_wasted_space_found = usize::MAX;

        for (index, texture) in self.first_render_pass_resources.fragment_shader_resource.texture_arrays.iter().enumerate()
        {
            let this_texture_wasted_space = texture.query_wasted_space(&texture_properties);

            if let Ok(this_texture_wasted_space) = this_texture_wasted_space
            {
                if this_texture_wasted_space < least_wasted_space_found
                {
                    most_suitable_array_index = Some(index);
                    least_wasted_space_found = this_texture_wasted_space;
                }
            }
        }

        return match most_suitable_array_index
        {
            Some(i) =>
                {
                    match self.first_render_pass_resources.fragment_shader_resource.texture_arrays[i].add_texture_sequentially_from_file_stbi(&texture_properties).unwrap()
                    {
                        TextureUploadResult::Success(index_offset) =>
                            {
                                let upload_info = UploadedTextureLocation
                                {
                                    array_index: i,
                                    index_offset,
                                    scale_x: 0.0,
                                    scale_y: 0.0
                                };

                                self.first_render_pass_resources.uploaded_textures.insert(texture_location, upload_info);

                                upload_info
                            },
                        TextureUploadResult::SuccessWithResize(index_offset, scale_x, scale_y) =>
                            {
                                let upload_info = UploadedTextureLocation
                                {
                                    array_index: i,
                                    index_offset,
                                    scale_x,
                                    scale_y
                                };

                                self.first_render_pass_resources.uploaded_textures.insert(texture_location, upload_info);

                                upload_info
                            },
                        _ => panic!()
                    }
                },
            None =>
                {
                    UploadedTextureLocation
                    {
                        array_index: 0,
                        index_offset: NO_SUITABLE_TEXTURE_STORAGE_INDEX,
                        scale_x: 1.0,
                        scale_y: 1.0
                    }
                }
        }
    }

    /// Creates mipmaps for the texture array associated with the given name
    ///
    /// `texture_array_name` - the name of the texture to create mipmaps for
    #[allow(dead_code)]
    pub fn create_texture_array_mipmaps<A: AsRef<str>>(&mut self, texture_array_name: A)
    {
        let texture_array_index = self.first_render_pass_resources.fragment_shader_resource.texture_lookup.get(texture_array_name.as_ref()).unwrap();
        self.first_render_pass_resources.fragment_shader_resource.texture_arrays[*texture_array_index].create_mipmaps();
    }

    /// Get the information to write to the indice buffer
    pub fn get_indice_mapped_buffer(&mut self) -> BufferWriteInfo
    {
        self.first_render_pass_resources.vertex_shader_resource.indice_buffer.as_mut().unwrap().wait_for_next_free_buffer(1_000_000).unwrap()
    }

    /// Flush the indice buffer at the given range
    ///
    /// `(start_byte_changed, num_bytes_changed)` - tuple representing range of indice buffer,
    ///                                             [start_byte_changed, start_byte_changed + num_bytes_changed],
    ///                                             to flush
    pub fn flush_indice_buffer(&mut self, (start_byte_changed, num_bytes_changed): (StartBufferChangedBytes, NumberBytesChanged))
    {
        self.first_render_pass_resources.vertex_shader_resource.indice_buffer.as_mut().unwrap().mark_buffer_updates_finish(start_byte_changed, num_bytes_changed);
    }

    /// Set the fences for all of the model mapped buffers. Call this at the end of the draw function
    pub fn set_fences_for_model_buffers(&mut self)
    {
        for x in &mut self.first_render_pass_resources.vertex_shader_resource.per_model_buffers
        {
            x.set_fence();
        }
    }

    /// Set the fences for all of the instanced mapped buffers. Call this at the end of the draw function
    pub fn set_fences_for_instance_buffers(&mut self)
    {
        for x in &mut self.first_render_pass_resources.vertex_shader_resource.per_instance_buffers
        {
            x.set_fence();
        }
    }

    /// Set the fences for the indice mapped buffers. Call this at the end of the draw function
    pub fn set_fence_for_indice_buffer(&mut self)
    {
        self.first_render_pass_resources.vertex_shader_resource.indice_buffer.as_mut().unwrap().set_fence();
    }

    /// Executes the first and render pass with the supplied draw parameters
    ///
    /// `in_draw_param` - structure holding variables required to execute the render passes
    pub fn draw(&mut self, in_draw_param: DrawPreparationParameters)
    {
        self.first_render_pass_resources.shader_program.use_shader_program();

        {
            for x in &mut self.first_render_pass_resources.fragment_shader_resource.texture_arrays
            {
                x.bind_texture_to_texture_unit();
            }
            for (_, cubemap) in &mut self.first_render_pass_resources.fragment_shader_resource.cube_maps
            {
                cubemap.bind();
            }

            let uniform_buffer_info = UniformBufferInformation
            {
                uniform_location: &self.first_render_pass_resources.uniform_resources.uniform_location_map,
                uniform_type: &self.first_render_pass_resources.uniform_resources.uniform_type_ids,
                buffers: &mut self.first_render_pass_resources.uniform_resources.mapped_buffers,
                buffers_to_flush: Vec::new(),
                buffers_to_fence: Vec::new()
            };

            let uniform_ecs = UniformECS
            {
                uniform_entities: &self.first_render_pass_resources.uniform_resources.uniform_entities,
                ecs: &self.first_render_pass_resources.uniform_resources.ecs
            };

            let mut first_render_pass_draw_param = DrawBuilderSystem::new()
                .with_uniforms(uniform_buffer_info)
                .with_uniform_entities(uniform_ecs)
                .with_model_info(&self.model_rendering_information)
                .with_level_of_views(&self.level_of_views)
                .with_name_lookup(&self.name_model_id_lookup)
                .with_camera(in_draw_param.camera)
                .with_logical_entities(in_draw_param.logical_ecs)
                .with_tree(in_draw_param.tree)
                .with_logical_lookup(in_draw_param.logical_entity_lookup)
                .with_render_system(self.first_render_pass_resources.shader_program.shader_program)
                .with_input_history(in_draw_param.input_history)
                .with_fbos(&mut self.draw_fn_accessible_fbo)
                .initially_drawing_skybox(false)
                .build();

            if let Some(ref mut first_render_fbo) = self.first_render_pass_resources.deferred_rendering_fbo
            {
                first_render_fbo.bind_fbo(BindingTarget::DrawFrameBuffer);
                unsafe
                    {
                        gl::StencilMask(0xFF);
                        gl::StencilFunc(gl::ALWAYS, LIT_SOURCE_STENCIL_VALUE, 0xFF);
                        gl::StencilOp(gl::KEEP, gl::KEEP, gl::REPLACE);
                        gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
                    }
            }

            if self.upload_local_lights
            {
                RenderSystem::upload_shadow_maps(&mut first_render_pass_draw_param, in_draw_param.upload_matrices,  in_draw_param.upload_view_matrices, in_draw_param.upload_indexes);
            }

            if let Some(shadow_map_binding) = self.first_render_pass_resources.shadow_map_binding_point
            {
                in_draw_param.shadow_fbo.bind_depth_texture_to_specific_texture_unit(shadow_map_binding);
            }

            (self.draw_function)(&mut first_render_pass_draw_param);

            unsafe{ gl::StencilFunc(gl::ALWAYS, 0x00, 0xFF); }

            (self.light_source_draw_function)(&mut first_render_pass_draw_param);

            if self.is_using_skybox
            {
                first_render_pass_draw_param.toggle_rendering_skybox(true);
                first_render_pass_draw_param.write_uniform_value("projectionMatrix", vec![first_render_pass_draw_param.get_camera().get_projection_matrix()]);
                first_render_pass_draw_param.write_uniform_value("viewMatrix", vec![first_render_pass_draw_param.get_camera().get_view_matrix()]);
                first_render_pass_draw_param.write_uniform_value("cameraLocation", vec![first_render_pass_draw_param.get_camera().get_position()]);

                let mod_view_matrix = nalgebra_glm::mat3_to_mat4(&nalgebra_glm::mat4_to_mat3(&first_render_pass_draw_param.get_camera().get_view_matrix()));
                first_render_pass_draw_param.write_uniform_value("viewMatrix", vec![mod_view_matrix]);

                first_render_pass_draw_param.write_uniform_value::<&str, i32>("renderingSkybox", vec![1]);
                unsafe{ gl::DepthFunc(gl::LEQUAL);  }
                first_render_pass_draw_param.flush_uniform_buffer();
                first_render_pass_draw_param.draw_skybox();
                first_render_pass_draw_param.set_fence_uniform_buffer();
                unsafe{ gl::DepthFunc(gl::LESS);  }
            }

            (self.transparency_draw_function)(&mut first_render_pass_draw_param);

            unsafe
                {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
                    gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
                }

            if let Some(ref mut second_pass_render) = self.second_render_pass_resources
            {
                if let Some(ref mut first_render_fbo) = self.first_render_pass_resources.deferred_rendering_fbo
                {
                    first_render_fbo.bind_colour_textures(vec![0, 1, 2, 3]);
                    first_render_fbo.bind_fbo(BindingTarget::ReadFrameBuffer);

                    unsafe
                        {
                            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0);
                            gl::BlitFramebuffer(0, 0, 1280, 720, 0, 0, 1280, 720, gl::STENCIL_BUFFER_BIT, gl::NEAREST);
                        }
                }

                let uniform_buffer_info = UniformBufferInformation
                {
                    uniform_location: &second_pass_render.uniform_resources.uniform_location_map,
                    uniform_type: &second_pass_render.uniform_resources.uniform_type_ids,
                    buffers: &mut second_pass_render.uniform_resources.mapped_buffers,
                    buffers_to_flush: Vec::new(),
                    buffers_to_fence: Vec::new()
                };

                let uniform_ecs = UniformECS
                {
                    uniform_entities: &second_pass_render.uniform_resources.uniform_entities,
                    ecs: &second_pass_render.uniform_resources.ecs
                };

                let mut second_render_pass_draw_param = DrawBuilderSystem::new()
                    .with_uniforms(uniform_buffer_info)
                    .with_uniform_entities(uniform_ecs)
                    .with_model_info(&self.model_rendering_information)
                    .with_level_of_views(&self.level_of_views)
                    .with_name_lookup(&self.name_model_id_lookup)
                    .with_camera(in_draw_param.camera)
                    .with_logical_entities(in_draw_param.logical_ecs)
                    .with_tree(in_draw_param.tree)
                    .with_logical_lookup(in_draw_param.logical_entity_lookup)
                    .with_render_system(second_pass_render.shader_program.shader_program)
                    .with_input_history(in_draw_param.input_history)
                    .with_fbos(&mut self.draw_fn_accessible_fbo)
                    .initially_drawing_skybox(false)
                    .build();

                second_pass_render.vao.bind();
                second_pass_render.shader_program.use_shader_program();

                if let Some(shadow_map_binding) = second_pass_render.shadow_map_binding_point
                {
                    in_draw_param.shadow_fbo.bind_depth_texture_to_specific_texture_unit(shadow_map_binding);
                }

                let mut any_light_source_visible = false;

                if self.upload_local_lights
                {
                    // TODO: Add constant directional light. Otherwise if no light sources are visible,
                    // TODO: change in colours will be very abrupt as texturing without lighting is used

                    any_light_source_visible |= RenderSystem::upload_directional_lights(&mut self.previous_directional_lights, in_draw_param.visible_sections_light, &mut second_render_pass_draw_param, in_draw_param.visible_directional_lights, self.max_num_lights.directional);
                    any_light_source_visible |= RenderSystem::upload_point_lights(&mut self.previous_point_lights, in_draw_param.visible_sections_light, &mut second_render_pass_draw_param, in_draw_param.visible_point_lights,self.max_num_lights.point);
                    any_light_source_visible |= RenderSystem::upload_spot_lights(&mut self.previous_spot_lights, in_draw_param.visible_sections_light, &mut second_render_pass_draw_param, in_draw_param.visible_spot_lights, self.max_num_lights.spot);
                }

                unsafe
                    {
                        gl::StencilFunc(gl::EQUAL, LIT_SOURCE_STENCIL_VALUE, 0xFF);
                        second_render_pass_draw_param.write_uniform_value("noLightSourceCutoff", vec![self.no_light_source_cutoff]);
                        second_render_pass_draw_param.write_uniform_value("defaultDiffuseFactor", vec![self.default_diffuse_factor]);
                        second_render_pass_draw_param.write_uniform_value("renderSkybox", vec![0_u32]);
                        second_render_pass_draw_param.write_uniform_value("renderingLightVolumes", vec![0_u32]);
                        second_render_pass_draw_param.write_uniform_value("cameraPosition", vec![in_draw_param.camera.get_position()]);
                        second_render_pass_draw_param.write_uniform_value("anyLightSourceVisible", vec![any_light_source_visible as u32]);
                        second_render_pass_draw_param.flush_uniform_buffer();
                        gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
                        second_render_pass_draw_param.set_fence_uniform_buffer();

                        gl::StencilFunc(gl::EQUAL, 0x00, 0xFF);
                        second_render_pass_draw_param.write_uniform_value("renderSkybox", vec![1_u32]);
                        second_render_pass_draw_param.write_uniform_value("renderingLightVolumes", vec![0_u32]);
                        second_render_pass_draw_param.flush_uniform_buffer();
                        gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
                        second_render_pass_draw_param.set_fence_uniform_buffer();
                        gl::Enable(gl::DEPTH_TEST);
                    }
            }
        }

        self.set_fences_for_instance_buffers();
        self.set_fences_for_model_buffers();
        self.set_fence_for_indice_buffer();
    }

    /// Register a model with this render system, allowing it to be referenced in the draw function
    ///
    /// `model_name` - the name of the model to use when rendering
    /// `model_id` - the id of model given when it was created
    /// `custom_level_of_view` - optional level of views describing what geometrical representation to use
    ///                          for rendering the model given its distance from the camera
    pub fn register_model<A: Into<String>>(&mut self, model_name: A, model_id: ModelId, custom_level_of_view: Option<Vec<LevelOfView>>, uses_texture: bool)
    {
        let name = model_name.into();
        self.name_model_id_lookup.insert(name.clone(), ModelNameLookupResult{ model_id, uses_texture });
        self.model_id_name_lookup.insert(model_id, name);

        if let Some(level_of_views) = custom_level_of_view
        {
            self.level_of_views.custom.insert(model_id, level_of_views);
        }
    }

    pub fn remove_model(&mut self, model_id: ModelId)
    {
        let model_name = self.model_id_name_lookup.get(&model_id).unwrap();

        if let Some(model_id) = self.name_model_id_lookup.remove(model_name)
        {
            self.level_of_views.custom.remove(&model_id.model_id);
        }

        self.model_id_name_lookup.remove(&model_id);
    }

    /// Get the indexes of the layouts in this render system shader program that correspond to instanced data
    pub fn get_instance_layout_indexes(&self) -> Vec<u32>
    {
        self.first_render_pass_resources.vertex_shader_resource.instance_layout_indexes.clone()
    }

    /// Get the indexes of the layouts in this render system shader program that correspond to model data
    pub fn get_model_layout_indexes(&self) -> Vec<u32>
    {
        self.first_render_pass_resources.vertex_shader_resource.model_layout_indexes.clone()
    }

    /// Update the geometry associated with models to match the correct instance range of instances
    /// of those models
    ///
    /// `updated_rendering_info` - map containing the updated model geometry of models to render
    pub fn update_model_rendering_info(&mut self, mut updated_rendering_info: HashMap<ModelId, ModelRenderingInformation>)
    {
        // The updated instance ranges were written directly to this render system's model_rendering_information.
        // These instance ranges are written then to the passed in updated_rendering_info variable, which
        // contains the geometry of models to render. Afterwards this variable has all required information
        // to render models, which is then assigned to this render system

        for (model_id, geometry_info) in mem::take(&mut self.model_rendering_information)
        {
            if let Some(i) = updated_rendering_info.get_mut(&model_id)
            {
                i.instance_location = geometry_info.instance_location;
            }
        }

        self.model_rendering_information = updated_rendering_info;
    }

    /// Determines if this render system requires shadows
    pub fn require_shadows(&self) -> bool
    {
        let second_pass_needs_shadows = if let Some(ref second_pass_resources) = self.second_render_pass_resources
        {
            second_pass_resources.shadow_map_binding_point.is_some()
        }
        else
        {
            false
        };

        self.first_render_pass_resources.shadow_map_binding_point.is_some() || second_pass_needs_shadows
    }

    /// Uploads nearby/visible directional lights to the second render pass uniforms, and marks the
    /// the lights as being rendered for use in the shadow flow
    ///
    /// `draw_param` - the variable required to query nearby lights and upload them as uniforms
    /// `directional_lights` - map of entity ids that identify directional lights
    fn upload_directional_lights(previous_directional_lights: &mut HashSet<EntityId>, visible_world_sections: &HashSet<UniqueWorldSectionId>, draw_param: &mut DrawParam,
                                 directional_lights: &mut HashSet::<EntityId>, max_direction_lights: u16) -> AnyLightSourceVisible
    {
        let visible_directional_lights = shadow_flow::find_nearby_lights
            (
                visible_world_sections,
                draw_param.get_bounding_box_tree(),
                FindLightType::Directional,
            );

        if visible_directional_lights.is_empty()
        {
            return false;
        }

        let number_rendered_directional_lights = visible_directional_lights.len().min(max_direction_lights as usize);
        let mut light_upload_information = LightUploadInformation::new(max_direction_lights as usize);

        let existing_lights = previous_directional_lights.intersection(&visible_directional_lights).map(|x| *x).collect::<HashSet<EntityId>>();
        previous_directional_lights.clear();

        for (index, directional_light) in existing_lights.iter().chain(visible_directional_lights.iter()).take(number_rendered_directional_lights).enumerate()
        {
            let light_info = draw_param.get_logical_ecs().get_ref::<LightInformation>(*directional_light).unwrap();

            light_upload_information.directions[index] = light_info.direction.unwrap();
            light_upload_information.diffuse_colours[index] = light_info.diffuse_colour;
            light_upload_information.specular_colours[index] = light_info.specular_colour;
            light_upload_information.ambient_colours[index] = light_info.ambient_colour;

            previous_directional_lights.insert(*directional_light);
        }

        draw_param.write_uniform_value("directionLightDir", light_upload_information.directions);
        draw_param.write_uniform_value("directionLightDiffuseColour", light_upload_information.diffuse_colours);
        draw_param.write_uniform_value("directionLightSpecularColour", light_upload_information.specular_colours);
        draw_param.write_uniform_value("directionLightAmbientColour", light_upload_information.ambient_colours);
        draw_param.write_uniform_value("numberDirectionLights", vec![number_rendered_directional_lights as u32]);

        // This map is looked at the shadow flow when determining what lights need to have a shadow map
        // created for them; lights being rendered have a priority
        directional_lights.extend(visible_directional_lights.iter());
        true
    }

    /// Uploads nearby/visible point lights to the second render pass uniforms, and marks the
    /// the lights as being rendered for use in the shadow flow
    ///
    /// `draw_param` - the variable required to query nearby lights and upload them as uniforms
    /// `directional_lights` - map of entity ids that identify point lights
    fn upload_point_lights(previous_point_lights: &mut HashSet<EntityId>, visible_world_sections: &HashSet::<UniqueWorldSectionId>, draw_param: &mut DrawParam,
                           point_lights: &mut HashSet::<EntityId>, max_point_lights: u16)  -> AnyLightSourceVisible
    {
        let visible_point_lights = shadow_flow::find_nearby_lights
            (
                visible_world_sections,
                draw_param.get_bounding_box_tree(),
                FindLightType::Point,
            );

        if visible_point_lights.is_empty()
        {
            return false;
        }

        let number_rendered_point_lights = visible_point_lights.len().min(max_point_lights as usize);
        let mut light_upload_information = LightUploadInformation::new(max_point_lights as usize);

        let existing_lights = previous_point_lights.intersection(&visible_point_lights).map(|x| *x).collect::<HashSet<EntityId>>();
        previous_point_lights.clear();

        for (index, point_light) in existing_lights.iter().chain(visible_point_lights.iter()).take(number_rendered_point_lights).enumerate()
        {
            let light_info = draw_param.get_logical_ecs().get_ref::<LightInformation>(*point_light).unwrap();
            let position = draw_param.get_logical_ecs().get_ref::<Position>(*point_light).unwrap();

            light_upload_information.positions[index] = position.get_position();
            light_upload_information.diffuse_colours[index] = light_info.diffuse_colour;
            light_upload_information.specular_colours[index] = light_info.specular_colour;
            light_upload_information.ambient_colours[index] = light_info.ambient_colour;
            light_upload_information.linear_coefficients[index] = light_info.linear_coefficient;
            light_upload_information.quadratic_coefficients[index] = light_info.quadratic_coefficient;
            light_upload_information.directions[index] = light_info.direction.unwrap();
            light_upload_information.fov[index] = light_info.fov.unwrap();
            light_upload_information.cutoff[index] = light_info.cutoff.unwrap();
            light_upload_information.outer_cutoff[index] = light_info.outer_cutoff.unwrap();

            previous_point_lights.insert(*point_light);
        }

        draw_param.write_uniform_value("pointLightPosition", light_upload_information.positions);
        draw_param.write_uniform_value("pointLightDirection", light_upload_information.directions);
        draw_param.write_uniform_value("pointLightDiffuseColour", light_upload_information.diffuse_colours);
        draw_param.write_uniform_value("pointLightSpecularColour", light_upload_information.specular_colours);
        draw_param.write_uniform_value("pointLightAmbientColour", light_upload_information.ambient_colours);
        draw_param.write_uniform_value("pointLightLinearCoefficient", light_upload_information.linear_coefficients);
        draw_param.write_uniform_value("pointLightQuadraticCoefficient", light_upload_information.quadratic_coefficients);
        draw_param.write_uniform_value("cutOff", light_upload_information.cutoff);
        draw_param.write_uniform_value("outerCutoff", light_upload_information.outer_cutoff);
        draw_param.write_uniform_value("numberPointLights", vec![number_rendered_point_lights as u32]);

        // This map is looked at the shadow flow when determining what lights need to have a shadow map
        // created for them; lights being rendered have a priority
        point_lights.extend(visible_point_lights.iter());
        true
    }

    /// Uploads nearby/visible spot lights to the second render pass uniforms, and marks the
    /// the lights as being rendered for use in the shadow flow
    ///
    /// `draw_param` - the variable required to query nearby lights and upload them as uniforms
    /// `directional_lights` - map of entity ids that identify spot lights
    fn upload_spot_lights(previous_spot_lights: &mut HashSet<EntityId>, visible_world_sections: &HashSet<UniqueWorldSectionId>, draw_param: &mut DrawParam,
                          spot_lights: &mut HashSet::<EntityId>, max_spot_lights: u16) -> AnyLightSourceVisible
    {
        let visible_spot_lights = shadow_flow::find_nearby_lights
            (
                visible_world_sections,
                draw_param.get_bounding_box_tree(),
                FindLightType::Spot,
            );

        if visible_spot_lights.is_empty()
        {
            return false;
        }

        let number_rendered_spot_lights = visible_spot_lights.len().min(max_spot_lights as usize);
        let mut light_upload_information = LightUploadInformation::new(max_spot_lights as usize);

        let existing_lights = previous_spot_lights.intersection(&visible_spot_lights).map(|x| *x).collect::<HashSet<EntityId>>();
        previous_spot_lights.clear();

        for (index, spot_light) in existing_lights.iter().chain(visible_spot_lights.iter()).take(number_rendered_spot_lights).enumerate()
        {
            let light_info = draw_param.get_logical_ecs().get_ref::<LightInformation>(*spot_light).unwrap();
            let position = draw_param.get_logical_ecs().get_ref::<Position>(*spot_light).unwrap();

            light_upload_information.positions[index] = position.get_position();
            light_upload_information.diffuse_colours[index] = light_info.diffuse_colour;
            light_upload_information.specular_colours[index] = light_info.specular_colour;
            light_upload_information.ambient_colours[index] = light_info.ambient_colour;
            light_upload_information.linear_coefficients[index] = light_info.linear_coefficient;
            light_upload_information.quadratic_coefficients[index] = light_info.quadratic_coefficient;
            light_upload_information.light_radius[index] = light_info.radius;
            let volume_info = vec4(position.get_position().x, position.get_position().y, position.get_position().z, light_info.radius);
            light_upload_information.light_volume_information[index] = volume_info;

            previous_spot_lights.insert(*spot_light);
        }

        draw_param.write_uniform_value("spotLightPosition", light_upload_information.positions);
        draw_param.write_uniform_value("spotLightDiffuseColour", light_upload_information.diffuse_colours);
        draw_param.write_uniform_value("spotLightSpecularColour", light_upload_information.specular_colours);
        draw_param.write_uniform_value("spotLightAmbientColour", light_upload_information.ambient_colours);
        draw_param.write_uniform_value("spotLightLinearCoefficient", light_upload_information.linear_coefficients);
        draw_param.write_uniform_value("spotLightQuadraticCoefficient", light_upload_information.quadratic_coefficients);
        draw_param.write_uniform_value("spotLightRadius", light_upload_information.light_radius);
        draw_param.write_uniform_value("numberSpotLights", vec![light_upload_information.number_lights as u32]);

        // This map is looked at the shadow flow when determining what lights need to have a shadow map
        // created for them; lights being rendered have a priority
        spot_lights.extend(visible_spot_lights.iter());
        true
    }

    fn upload_shadow_maps(draw_param: &mut DrawParam, matrices: &Vec<TMat4<f32>>, view_matrices: &Vec<TMat4<f32>>, indexes: &Vec<u32>)
    {
        assert_eq!(matrices.len(), indexes.len());

        let mut matrices_data = matrices.clone();

        while matrices_data.len() < 18
        {
            matrices_data.push(TMat4::default());
        }

        let matrices_data = matrices_data.iter().take(6).map(|x| *x).collect::<Vec<TMat4<f32>>>();

        draw_param.write_uniform_mat4_stall("correctMatrix", matrices_data[1]);
        draw_param.write_uniform_value("lightMatrices", matrices_data.clone());

        let mut view_data = view_matrices.clone();

        while view_data.len() < 18
        {
            view_data.push(TMat4::default());
        }

        let view_data = view_data.iter().take(6).map(|x| *x).collect::<Vec<TMat4<f32>>>();
        draw_param.write_uniform_value("lightViewMatrices", view_data.clone());


        draw_param.write_uniform_value("numberLightMatrices", vec![matrices_data.len() as u32]);



        let mut index_data = indexes.clone();

        while index_data.len() < 18
        {
            index_data.push(0);
        }
    }
}

/// Helper structure to hold all required data for uploading data to light uniforms
pub struct LightUploadInformation
{
    positions: Vec<TVec3<f32>>,
    diffuse_colours: Vec<TVec3<f32>>,
    specular_colours: Vec<TVec3<f32>>,
    ambient_colours: Vec<TVec4<f32>>,
    linear_coefficients: Vec<f32>,
    quadratic_coefficients: Vec<f32>,
    directions: Vec<TVec3<f32>>,
    fov: Vec<f32>,
    cutoff: Vec<f32>,
    outer_cutoff: Vec<f32>,
    light_radius: Vec<f32>,
    number_lights: usize,
    light_volume_information: Vec<TVec4<f32>>
}

impl LightUploadInformation
{
    /// Creates a new LightUploadInformation with enough space to store the given number of lights.
    /// Default values for all light information is 0
    ///
    /// `number_lights` - the number of lights to reserve space for
    pub fn new(number_lights: usize) -> LightUploadInformation
    {
        LightUploadInformation
        {
            positions: vec![vec3(0.0, 0.0, 0.0); number_lights],
            diffuse_colours: vec![vec3(0.0, 0.0, 0.0); number_lights],
            specular_colours: vec![vec3(0.0, 0.0, 0.0); number_lights],
            ambient_colours: vec![vec4(0.0, 0.0, 0.0, 0.0); number_lights],
            linear_coefficients: vec![0.0; number_lights],
            quadratic_coefficients: vec![0.0; number_lights],
            directions: vec![vec3(0.0, 0.0, 0.0); number_lights],
            fov: vec![0.0; number_lights],
            cutoff: vec![0.0; number_lights],
            outer_cutoff: vec![0.0; number_lights],
            light_radius: vec![0.0; number_lights],
            number_lights,
            light_volume_information: vec![vec4(0.0, 0.0, 0.0, 0.0); number_lights],
        }
    }
}