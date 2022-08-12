use std::any::TypeId;
use std::ffi::{c_void, CString};
use std::fmt::Debug;
use std::mem::size_of;
use hashbrown::HashMap;
use nalgebra_glm::{TMat4x4, TVec3, TVec4};
use serde::{Deserialize, Serialize};
use crate::exports::camera_object::Camera;
use crate::flows::render_flow::{InstanceRange, ModelRenderingInformation};
use crate::models::model_definitions::ModelId;
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::render_components::frame_buffer::FBO;
use crate::render_components::mapped_buffer::MappedBuffer;
use crate::render_system::render_pass_resources::UniformBufferInformation;
use crate::render_system::render_system::{LevelOfViews, ModelNameLookupResult, UniformECS};
use crate::window::input_state::InputHistory;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;

/// Holds information regarding what model to draw, and more specifically what instances of that
/// model to draw
pub struct ModelDrawCommand<A: AsRef<str>>
{
    pub model_name: A,
    pub component_indexes: Vec<usize>,
    pub render_sortable_together: bool,
    pub is_program_generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelOfView
{
    pub min_distance: f32,
    pub max_distance: f32,
}

/// Holds variables required to execute a render function

pub struct DrawParam<'a>
{
    uniforms: UniformBufferInformation<'a>,
    uniform_entities: UniformECS<'a>,
    model_rendering_information: &'a HashMap<ModelId, ModelRenderingInformation>,
    level_of_views: &'a LevelOfViews,
    name_model_id_lookup: &'a HashMap<String, ModelNameLookupResult>,
    camera: &'a Camera,
    logical_entities: &'a ECS,
    tree: &'a BoundingBoxTree,
    render_system: u32,
    input_history: &'a InputHistory,
    draw_fn_accessible_fbo: &'a mut HashMap<String, FBO>,
    rendering_skybox: bool,
}

impl<'a> DrawParam<'a>
{
    pub fn toggle_rendering_skybox(&mut self, rendering_skybox: bool)
    {
        self.rendering_skybox = rendering_skybox;
    }

    /// Writes a matrix of floats (4x4) to the active shader program
    ///
    /// `uniform_name` - the name of the uniform
    /// `data` - the matrix to upload to the given uniform

    pub fn write_uniform_mat4_stall<A: AsRef<str>>(&mut self, uniform_name: A, data: TMat4x4<f32>)
    {
        unsafe
            {
                let c_string = CString::new(uniform_name.as_ref()).unwrap();
                gl::UniformMatrix4fv(gl::GetUniformLocation(self.render_system, c_string.as_ptr()),
                                     1, gl::FALSE, data.as_ptr());
            }
    }

    /// Writes an unsigned int to the active shader program
    ///
    /// `uniform_name` - the name of the uniform
    /// `data` - the unsigned int to upload to the given uniform

    pub fn write_uint<A: AsRef<str>>(&mut self, uniform_name: A, data: u32)
    {
        unsafe
            {
                let c_string = CString::new(uniform_name.as_ref()).unwrap();
                gl::Uniform1ui(gl::GetUniformLocation(self.render_system, c_string.as_ptr()), data);
            }
    }

    /// Writes an int to the active shader program
    ///
    /// `uniform_name` - the name of the uniform
    /// `data` - the int to upload to the given uniform

    pub fn write_int<A: AsRef<str>>(&mut self, uniform_name: A, data: i32)
    {
        unsafe
            {
                let c_string = CString::new(uniform_name.as_ref()).unwrap();
                gl::Uniform1i(gl::GetUniformLocation(self.render_system, c_string.as_ptr()), data);
            }
    }

    /// Writes a vector of floats (3D) to the active shader program
    ///
    /// `uniform_name` - the name of the uniform
    /// `data` - the 3D vector of floats to upload to the given uniform

    pub fn write_vec3<A: AsRef<str>>(&mut self, uniform_name: A, data: TVec3<f32>)
    {
        unsafe
            {
                let c_string = CString::new(uniform_name.as_ref()).unwrap();
                gl::Uniform3fv(gl::GetUniformLocation(self.render_system, c_string.as_ptr()), 1, data.as_ptr());
            }
    }

    /// Writes a vector of floats (4D) to the active shader program
    ///
    /// `uniform_name` - the name of the uniform
    /// `data` - the 4D vector of floats to upload to the given uniform

    pub fn write_vec4<A: AsRef<str>>(&mut self, uniform_name: A, data: TVec4<f32>)
    {
        unsafe
            {
                let c_string = CString::new(uniform_name.as_ref()).unwrap();
                gl::Uniform4fv(gl::GetUniformLocation(self.render_system, c_string.as_ptr()), 1, data.as_ptr());
            }
    }

    /// Get the FBO associated with the given name
    ///
    /// `fbo_name` - the name of the FBO to return

    pub fn get_fbo<A: AsRef<str>>(&mut self, fbo_name: A) -> Option<&mut FBO>
    {
        self.draw_fn_accessible_fbo.get_mut(fbo_name.as_ref())
    }

    /// Writes the provided data to the uniform specified. This function does not stall- ie no stalling
    /// OpenGL functions are called
    ///
    /// `uniform_name` - the name of the uniform to upload to
    /// `data` - the data to upload to the uniform with the name provided
    pub fn write_uniform_value<A: AsRef<str>, T: 'static + Debug>(&mut self, uniform_name: A, data: Vec<T>)
    {
        let uniform_location = match self.uniforms.uniform_location.get(uniform_name.as_ref())
        {
            Some(i) => i,
            None => panic!("Failed to find uniform: {}", uniform_name.as_ref())
        };
        let buffer = &mut self.uniforms.buffers[uniform_location.mapped_buffer_index];
        let write_info = buffer.wait_for_next_free_buffer(5_000_000).unwrap();

        let mut offset_bytes = uniform_location.offset_bytes;

        let type_id = TypeId::of::<T>();

        let expected_info = self.uniforms.uniform_type.get(uniform_name.as_ref()).unwrap();
        debug_assert_eq!(expected_info.type_id, type_id, "Incorrect data type supplied for uniform: {}", uniform_name.as_ref());
        debug_assert_eq!(expected_info.num_elements as usize, data.len(), "Incorrect number of elements for uniform: {}. Expected {}, found {}",
                         uniform_name.as_ref(), expected_info.num_elements, data.len());


        for (index, value) in data.into_iter().enumerate()
        {
            MappedBuffer::write_single_serialized_value(write_info, value, offset_bytes, false);

            // (index + 1) as the offset_bytes will be used for the next iteration of the loop
            offset_bytes = uniform_location.offset_bytes +  // Base offset in bytes for the uniform
                (index + 1) as isize * uniform_location.sub_padding_bytes +  // Adjust byte count for padding
                ((index + 1) * size_of::<T>()) as isize; // Adjust byte count assuming no padding
        }

        if self.uniforms.buffers_to_flush.iter().find(|x| **x == uniform_location.mapped_buffer_index).is_none()
        {
            self.uniforms.buffers_to_flush.push(uniform_location.mapped_buffer_index);
            self.uniforms.buffers_to_fence.push(uniform_location.mapped_buffer_index);
        }
    }

    /// Get the value of the uniform
    ///
    /// `entity_name` - the name of the uniform to get the value of
    pub fn get_uniform_entity_value<T: 'static + Serialize + Deserialize<'a>>(&self, entity_name: &str) -> Option<&T>
    {
        match self.uniform_entities.uniform_entities.get(&entity_name.to_string())
        {
            Some(i) => self.uniform_entities.ecs.get_ref::<T>(*i),
            None => None
        }
    }

    /// Get the camera used for rendering
    pub fn get_camera(&self) -> &Camera
    {
        self.camera
    }

    /// Mark to OpenGL that changes in uniform buffers have been made. Call this when all changes to
    /// uniforms for the current frame have been made
    pub fn flush_uniform_buffer(&mut self)
    {
        if !self.rendering_skybox && self.uniforms.uniform_location.contains_key("renderingSkybox")
        {
            self.write_uniform_value("renderingSkybox", vec![0]);
        }

        for x in &self.uniforms.buffers_to_flush
        {
            let bytes_to_flush = self.uniforms.buffers[*x].size_buffer_bytes;
            self.uniforms.buffers[*x].mark_buffer_updates_finish(0, bytes_to_flush);
        }

        self.uniforms.buffers_to_flush.clear();
    }

    /// Set fences for the uniform buffer. Call this after all draw calls for the draw function
    /// have been made
    pub fn set_fence_uniform_buffer(&mut self)
    {
        for x in &self.uniforms.buffers_to_fence
        {
            self.uniforms.buffers[*x].set_fence();
        }

        self.uniforms.buffers_to_fence.clear();
    }

    /// Get the input history

    pub fn get_input_history(&self) -> &InputHistory
    {
        self.input_history
    }

    pub fn draw_skybox(&mut self)
    {
        self.flush_uniform_buffer();

        self.write_uint("drawingModelsWithTextures", 1);

        let lookup_result = match self.name_model_id_lookup.get("skyBox")
        {
            Some(i) => i.clone(),
            None => panic!("Could not find model to draw: skyBox")
        };

        if let Some(rendering_info) = self.model_rendering_information.get(&lookup_result.model_id)
        {
            for mesh in &rendering_info.mesh_render_info
            {
                unsafe
                    {
                        gl::DrawElementsBaseVertex(gl::TRIANGLES, mesh.indice_count, gl::UNSIGNED_INT,
                                                   (mesh.indice_offset * size_of::<u32>()) as *const c_void,
                                                   mesh.vertex_offset);
                    }
            }
        }

        self.set_fence_uniform_buffer();
    }

    /// Models that are specified as input into this function are drawn
    ///
    /// `draw_commands` - the commands of the models to draw
    pub fn draw_model_with_sortable_index<A: AsRef<str>>(&mut self, draw_commands: Vec<ModelDrawCommand<A>>)
    {
        self.flush_uniform_buffer();

        let mut models_use_textures = Vec::new();
        let mut models_do_not_use_textures = Vec::new();

        for command in draw_commands
        {
            let lookup_result = match self.name_model_id_lookup.get(command.model_name.as_ref())
            {
                Some(i) => i.clone(),
                None =>
                    {
                        if !command.is_program_generated
                        {
                            panic!("Could not find model to draw: {}", command.model_name.as_ref())
                        }
                        else
                        {
                            continue;
                        }
                    }
            };

            if lookup_result.uses_texture
            {
                models_use_textures.push((lookup_result.model_id, command));
            }
            else
            {
                models_do_not_use_textures.push((lookup_result.model_id, command));
            }
        }

        self.write_uint("drawingModelsWithTextures", 1);
        self.render_models(models_use_textures);
        self.write_uint("drawingModelsWithTextures", 0);
        self.render_models(models_do_not_use_textures);

        // This is required if several draw calls are made per frame, and between those draw calls,
        // changes to uniform buffers are made
        self.set_fence_uniform_buffer();
    }

    fn render_models<A: AsRef<str>>(&mut self, draw_commands: Vec<(ModelId, ModelDrawCommand<A>)>)
    {
        for (model_id, command) in draw_commands
        {
            // Iterate over all of the possible level of views, and for each one check if there are instances
            // that need to be rendered

            let number_level_of_views = match self.level_of_views.custom.get(&model_id)
            {
                Some(i) => i.len(),
                None => self.level_of_views.default.len()
            };

            for x in 0..number_level_of_views
            {
                let mut adjusted_model_id = model_id;
                ModelId::apply_level_of_view(&mut adjusted_model_id.model_index, x as u32);

                if let Some(rendering_info) = self.model_rendering_information.get(&adjusted_model_id)
                {
                    let mut render_ranges: Vec<InstanceRange> = Vec::new();

                    // Merge adjacent ranges together to reduce draw calls
                    for sortable_component_index in &command.component_indexes
                    {
                        if let Some(range) = rendering_info.instance_location.get(sortable_component_index)
                        {
                            if command.render_sortable_together
                            {
                                // Current sortable index comes after accumulated range
                                if let Some(instance_range) = render_ranges.iter_mut().find(|x| x.begin_instance == range.begin_instance + range.count)
                                {
                                    instance_range.count += range.count;
                                    continue;
                                }

                                // Current sortable index comes before accumulated range
                                if let Some(instance_range) = render_ranges.iter_mut().find(|x| x.begin_instance + x.count == range.begin_instance)
                                {
                                    instance_range.begin_instance -= range.count;
                                    instance_range.count += range.count;
                                    continue;
                                }
                            }

                            render_ranges.push(*range);
                        }
                    }

                    for instances_to_render in render_ranges.iter().filter(|x| x.count != 0)
                    {
                        for mesh in &rendering_info.mesh_render_info
                        {
                            unsafe
                                {
                                    gl::DrawElementsInstancedBaseVertexBaseInstance
                                        (
                                            gl::TRIANGLES,
                                            mesh.indice_count,
                                            gl::UNSIGNED_INT,
                                            (mesh.indice_offset * size_of::<u32>()) as *const c_void,
                                            instances_to_render.count as i32,
                                            mesh.vertex_offset,
                                            instances_to_render.begin_instance,
                                        );
                                }
                        }
                    }
                }
            }
        }
    }

    /// Get the logical entities ECS
    pub fn get_logical_ecs(&self) -> &ECS
    {
        self.logical_entities
    }

    /// Get the tree that groups the entities based upon their position
    pub fn get_bounding_box_tree(&self) -> &BoundingBoxTree
    {
        self.tree
    }
}

pub struct DrawBuilderParam<'a>
{
    uniforms: Option<UniformBufferInformation<'a>>,
    uniform_entities: Option<UniformECS<'a>>,
    model_rendering_information: Option<&'a HashMap<ModelId, ModelRenderingInformation>>,
    level_of_views: Option<&'a LevelOfViews>,
    name_model_id_lookup: Option<&'a HashMap<String, ModelNameLookupResult>>,
    camera: Option<&'a Camera>,
    logical_entities: Option<&'a ECS>,
    tree: Option<&'a BoundingBoxTree>,
    logical_lookup: Option<&'a HashMap<String, EntityId>>,
    render_system: Option<u32>,
    input_history: Option<&'a InputHistory>,
    draw_fn_accessible_fbo: Option<&'a mut HashMap<String, FBO>>,
    initilally_rendering_skybox: bool,
}

pub struct DrawBuilderSystem<'a>(DrawBuilderParam<'a>);
pub struct UniformsBuilder<'a>(DrawBuilderParam<'a>);
pub struct UniformEntitiesBuilder<'a>(DrawBuilderParam<'a>);
pub struct ModelInformationBuilder<'a>(DrawBuilderParam<'a>);
pub struct LevelViewsBuilder<'a>(DrawBuilderParam<'a>);
pub struct NameModelLookupBuilder<'a>(DrawBuilderParam<'a>);
pub struct DrawParamCameraBuilder<'a>(DrawBuilderParam<'a>);
pub struct LogicalEntitiesBuilder<'a>(DrawBuilderParam<'a>);
pub struct TreeBuilder<'a>(DrawBuilderParam<'a>);
pub struct LogicalLookupBuilder<'a>(DrawBuilderParam<'a>);
pub struct RenderSystemBuilder<'a>(DrawBuilderParam<'a>);
pub struct InputHistoryBuilder<'a>(DrawBuilderParam<'a>);
pub struct DrawFBOBuilder<'a>(DrawBuilderParam<'a>);
pub struct CreateDrawParam<'a>(DrawBuilderParam<'a>);
pub struct InitiallyRenderingSkybox<'a>(DrawBuilderParam<'a>);

impl<'a> DrawBuilderSystem<'a>
{
    pub fn new() -> UniformsBuilder<'a>
    {
        UniformsBuilder
            (
                DrawBuilderParam
                {
                    uniforms: None,
                    uniform_entities: None,
                    model_rendering_information: None,
                    level_of_views: None,
                    name_model_id_lookup: None,
                    camera: None,
                    logical_entities: None,
                    tree: None,
                    logical_lookup: None,
                    render_system: None,
                    input_history: None,
                    draw_fn_accessible_fbo: None,
                    initilally_rendering_skybox: false,
                }
            )
    }
}

impl<'a> UniformsBuilder<'a>
{
    pub fn with_uniforms(mut self, uniforms: UniformBufferInformation<'a>) -> UniformEntitiesBuilder
    {
        self.0.uniforms = Some(uniforms);
        UniformEntitiesBuilder(self.0)
    }
}

impl<'a> UniformEntitiesBuilder<'a>
{
    pub fn with_uniform_entities(mut self, uniforms: UniformECS<'a>) -> ModelInformationBuilder
    {
        self.0.uniform_entities = Some(uniforms);
        ModelInformationBuilder(self.0)
    }
}

impl<'a> ModelInformationBuilder<'a>
{
    pub fn with_model_info(mut self, models: &'a HashMap<ModelId, ModelRenderingInformation>) -> LevelViewsBuilder
    {
        self.0.model_rendering_information = Some(models);
        LevelViewsBuilder(self.0)
    }
}

impl<'a> LevelViewsBuilder<'a>
{
    pub fn with_level_of_views(mut self, level_of_views: &'a LevelOfViews) -> NameModelLookupBuilder
    {
        self.0.level_of_views = Some(level_of_views);
        NameModelLookupBuilder(self.0)
    }
}

impl<'a> NameModelLookupBuilder<'a>
{
    pub fn with_name_lookup(mut self, name_lookup: &'a HashMap<String, ModelNameLookupResult>) -> DrawParamCameraBuilder
    {
        self.0.name_model_id_lookup = Some(name_lookup);
        DrawParamCameraBuilder(self.0)
    }
}

impl<'a> DrawParamCameraBuilder<'a>
{
    pub fn with_camera(mut self, camera: &'a Camera) -> LogicalEntitiesBuilder
    {
        self.0.camera = Some(camera);
        LogicalEntitiesBuilder(self.0)
    }
}

impl<'a> LogicalEntitiesBuilder<'a>
{
    pub fn with_logical_entities(mut self, entities: &'a ECS) -> TreeBuilder
    {
        self.0.logical_entities = Some(entities);
        TreeBuilder(self.0)
    }
}

impl<'a> TreeBuilder<'a>
{
    pub fn with_tree(mut self, tree: &'a BoundingBoxTree) -> LogicalLookupBuilder
    {
        self.0.tree = Some(tree);
        LogicalLookupBuilder(self.0)
    }
}

impl<'a> LogicalLookupBuilder<'a>
{
    pub fn with_logical_lookup(mut self, lookup: &'a HashMap<String, EntityId>) -> RenderSystemBuilder
    {
        self.0.logical_lookup = Some(lookup);
        RenderSystemBuilder(self.0)
    }
}

impl<'a> RenderSystemBuilder<'a>
{
    pub fn with_render_system(mut self, render_system: u32) -> InputHistoryBuilder<'a>
    {
        self.0.render_system = Some(render_system);
        InputHistoryBuilder(self.0)
    }
}

impl<'a> InputHistoryBuilder<'a>
{
    pub fn with_input_history(mut self, history: &'a InputHistory) -> DrawFBOBuilder
    {
        self.0.input_history = Some(history);
        DrawFBOBuilder(self.0)
    }
}

impl<'a> DrawFBOBuilder<'a>
{
    pub fn with_fbos(mut self, fbo_lookup: &'a mut HashMap<String, FBO>) -> InitiallyRenderingSkybox
    {
        self.0.draw_fn_accessible_fbo = Some(fbo_lookup);
        InitiallyRenderingSkybox(self.0)
    }
}

impl<'a> InitiallyRenderingSkybox<'a>
{
    pub fn initially_drawing_skybox(mut self, rendering_skybox: bool) -> CreateDrawParam<'a>
    {
        self.0.initilally_rendering_skybox = rendering_skybox;
        CreateDrawParam(self.0)
    }
}

impl<'a> CreateDrawParam<'a>
{
    pub fn build(self) -> DrawParam<'a>
    {
        DrawParam
        {
            uniforms: self.0.uniforms.unwrap(),
            uniform_entities: self.0.uniform_entities.unwrap(),
            model_rendering_information: self.0.model_rendering_information.unwrap(),
            level_of_views: self.0.level_of_views.unwrap(),
            name_model_id_lookup: self.0.name_model_id_lookup.unwrap(),
            camera: self.0.camera.unwrap(),
            logical_entities: self.0.logical_entities.unwrap(),
            tree: self.0.tree.unwrap(),
            render_system: self.0.render_system.unwrap(),
            input_history: self.0.input_history.unwrap(),
            draw_fn_accessible_fbo: self.0.draw_fn_accessible_fbo.unwrap(),
            rendering_skybox: self.0.initilally_rendering_skybox
        }
    }
}