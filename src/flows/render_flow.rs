use std::mem::size_of;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender};
use hashbrown::{HashMap, HashSet};
use lazy_static::lazy_static;
use nalgebra_glm::{TMat4, TVec3, TVec4, vec4};
use parking_lot::{Mutex, RwLock};
use rayon::iter::ParallelIterator;
use rayon::prelude::ParallelSlice;
use crate::exports::camera_object::Camera;
use crate::exports::logic_components::RenderSystemIndex;
use crate::exports::movement_components::TransformationMatrix;
use crate::exports::rendering::LevelOfView;
use crate::flows::shadow_flow;
use crate::flows::shadow_flow::{CalculationArgs, ShadowFlow, ShadowMapLocation};
use crate::helper_things::aabb_helper_functions::distance_to_aabb;
use crate::helper_things::cpu_usage_reducer::TimeTakeHistory;
use crate::helper_things::environment::get_asset_folder;
use crate::models::model_definitions::{MeshGeometry, ModelId};
use crate::models::model_storage::{ModelBank, ModelBankOwner};
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::render_components::frame_buffer::{AttachmentFormat, BindingTarget, FBO};
use crate::render_components::mapped_buffer::{BufferWriteInfo, MappedBuffer};
use crate::render_system::builder::{MaxLightConstraints, RenderSystemBuilder};
use crate::render_system::render_system::{LevelOfViews, ModelUpdateFunction, NumberBytesChanged, RenderSystem, StartBufferChangedBytes, UploadedTextureLocation};
use crate::render_system::system_information::{DrawFunction, DrawPreparationParameters, FragmentShaderInformation, GLSLVersion, IndiceInformation, LayoutInformation, LayoutInstance, LayoutType, LayoutUse, MagFilterOptions, MinFilterOptions, TextureFormat, TextureInformation, TextureWrap, Uniform, UniformBlock, UniformType, VertexShaderInformation};
use crate::{specify_model_geometry_layouts, specify_type_ids};
use crate::flows::visible_world_flow::CullResult;
use crate::window::input_state::InputHistory;
use crate::world::bounding_box_tree_v2::{BoundingBoxTree, SharedWorldSectionId, UniqueWorldSectionId};

lazy_static!
{
    static ref VISIBLE_WORLD_SECTIONS_HISTORY: Mutex<TimeTakeHistory> = Mutex::new(TimeTakeHistory::new());
}

/// ************ Helper Structures ******************

type SortableIndex = usize;

/// Stores information required to call the required draw function for a single model mesh
#[derive(Debug)]
pub struct MeshRenderingInformation
{
    pub indice_count: i32,
    pub vertex_offset: i32,
    pub indice_offset: usize,
}

impl MeshRenderingInformation
{
    /// Creates a new MeshRenderingInformation structure with default 0-values for all member variables
    fn new() -> MeshRenderingInformation
    {
        MeshRenderingInformation
        {
            indice_count: 0,
            vertex_offset: 0,
            indice_offset: 0,
        }
    }
}

/// Stores all of the model's mesh drawing information
#[derive(Debug)]
pub struct ModelRenderingInformation
{
    pub mesh_render_info: Vec<MeshRenderingInformation>,
    pub instance_location: HashMap<SortableIndex, InstanceRange>,
}

impl ModelRenderingInformation
{
    /// Initializes a new ModelRenderInformation with no mesh rendering information
    fn new() -> ModelRenderingInformation
    {
        ModelRenderingInformation
        {
            mesh_render_info: Vec::new(),
            instance_location: HashMap::default(),
        }
    }
}

/// Specifies a range into an instance buffer
#[derive(Debug, Copy, Clone)]
pub struct InstanceRange
{
    pub begin_instance: u32,
    pub count: u32,
}

/// Variables required to execute the render flow
pub struct RenderArguments<'a>
{
    pub visible_world_sections: CullResult,
    pub bounding_box_tree: &'a BoundingBoxTree,
    pub ecs: &'a ECS,
    pub camera: &'a Camera,
    pub model_bank_owner: Arc<RwLock<ModelBankOwner>>,
    pub input_history: &'a InputHistory
}

/// Keeps track of the information for instanced layouts that will be written to the appropriate
/// render system's buffers
#[derive(Debug, Clone)]
struct WrittenInformation
{
    number_entities: u32,
    layout_data: Vec<(u32, Vec<u8>)>,
}

/// Stores the information needed to make the render system ready to render models after models
/// were written to the render system's buffers
struct UpdateModelInfo
{
    indice_flush_info: (StartBufferChangedBytes, NumberBytesChanged),
    flush_info: Vec<(StartBufferChangedBytes, NumberBytesChanged)>,
    updated_rendering_info: HashMap<ModelId, ModelRenderingInformation>
}

/// *** Structures to sort entities in visible world sections ***

type SortResult = HashMap<ModelId, HashMap<usize, WrittenInformation>>;

/// Stores the parameters required to sort entities in visible world sections
pub struct SortWorldSectionEntitiesParam<'a>
{
    visible_world_sections: &'a CullResult,
    ecs: &'a ECS,
    bounding_box_tree: &'a BoundingBoxTree,
    unique_layout_indexes: Arc<Vec<u32>>,
    layout_update_function: fn(u32, &ECS, &mut Vec<u8>, EntityId),
    camera_position: TVec3<f32>,
    draw_distance: f32,
    level_views: &'a LevelOfViews
}

/// Variables required to sort entities in a specific world section(s)
struct SortWorldChunkArgs<'a>
{
    world_sections: &'a[UniqueWorldSectionId],
    sorting_param: &'a SortWorldSectionEntitiesParam<'a>,
    processed_world_sections: &'a Mutex<HashSet::<SharedWorldSectionId>>
}

/// Variables required to append sorted entity data to data structures that will be uploaded
/// to vRAM
struct AddEntitiesArgs<'a>
{
    entities: &'a HashSet::<EntityId>,
    sorting_param: &'a SortWorldSectionEntitiesParam<'a>,
    local_sorted_data: &'a mut SortResult,
    distance_sphere: f32,
    sortable_index: SortableIndex,
}

/// Stores data for static entities
#[derive(Debug)]
struct UniqueSectionData
{
    world_data: HashMap<UniqueWorldSectionId, HashMap<ModelId, HashMap<SortableIndex, WrittenInformation>>>,
    world_sections: HashSet::<UniqueWorldSectionId>
}

impl UniqueSectionData
{
    /// Create a new instance of UniqueSectionData with no static entity information
    fn new() -> UniqueSectionData
    {
        UniqueSectionData { world_data: HashMap::default(), world_sections: HashSet::default() }
    }
}

/// ************* Main Structure and Logic ***************

/// Handles the logic of uploading the correct data to the appropriate render system
pub struct RenderFlow
{
    tx: SyncSender<UpdateModelInfo>,
    rx: Receiver<UpdateModelInfo>,
    render_systems: Vec<RenderSystem>,
    static_data_unique_section: Arc<RwLock<Vec<UniqueSectionData>>>,

    visible_direction_lights: HashSet::<EntityId>,
    visible_point_lights: HashSet::<EntityId>,
    visible_spot_lights: HashSet::<EntityId>,

    shadow_flow: ShadowFlow,
    shadow_fbo: FBO,
    window_dimensions: (i32, i32),
    enable_shadow_rendering: bool,
}

impl RenderFlow
{
    /// Creates a new render flow
    ///
    /// `render_systems` - the render systems used for rendering
    /// `level_of_views` - the divisions of the field of view required for model detail adjustment based off
    ///                     distance to the camera
    /// `window_dimensions` - the initial window dimensions of the window being rendered to
    pub fn new(mut render_systems: Vec<RenderSystem>, no_light_source_cutoff: f32, default_diffuse_factor: f32, level_of_views: Vec<LevelOfView>, window_dimensions: (i32, i32),
               shadow_draw_fn: DrawFunction, shadow_light_draw_fn: DrawFunction, shadow_transparency_draw_function: DrawFunction) -> RenderFlow
    {
        // Only one result after uploading models into a render system
        let (tx, rx) = sync_channel(1);
        render_systems.push(RenderFlow::create_shadow_render_system(level_of_views, no_light_source_cutoff, default_diffuse_factor, shadow_draw_fn, shadow_light_draw_fn, shadow_transparency_draw_function));

        let enable_shadow_rendering = render_systems.iter().find(|x| x.require_shadows()).is_some();

        let static_data_unique_section = (0..render_systems.len())
            .into_iter()
            .map(|_| UniqueSectionData::new())
            .collect::<Vec<UniqueSectionData>>();

        let shadow_fbo_depth_texture = TextureInformation
        {
            sampler_name: "shadowMapTextures".to_string(),
            number_mipmaps: 1,
            format: TextureFormat::Depth,
            min_filter_options: MinFilterOptions::Nearest,
            mag_filter_options: MagFilterOptions::Nearest,
            wrap_s: TextureWrap::ClampToBorder,
            wrap_t: TextureWrap::ClampToBorder,
            width: 1024,
            height: 1024,
            number_textures: 6,
            border_color: Some(vec4(1.0, 1.0, 1.0, 1.0))
        };

        let shadow_fbo = FBO::new(vec![], Some(shadow_fbo_depth_texture), None, None).unwrap();
        unsafe{ gl::Viewport(0, 0, window_dimensions.0, window_dimensions.1); }

        RenderFlow{ tx, rx, render_systems, visible_direction_lights: HashSet::default(),
            visible_point_lights: HashSet::default(), visible_spot_lights: HashSet::default(),
            shadow_flow: ShadowFlow::new(6), shadow_fbo, window_dimensions, enable_shadow_rendering,
            static_data_unique_section: Arc::new(RwLock::new(static_data_unique_section)) }
    }

    /// Updates render system to hold correct data for rendering and starts the drawing logic
    ///
    /// `render_args` - structure containing the required variables for rendering
    pub fn render(&mut self, render_args: RenderArguments)
    {
        let visible_sections_light = shadow_flow::find_nearby_world_sections_maps
            (
                render_args.camera.get_position(),
                render_args.camera.get_far_draw_distance(),
                render_args.bounding_box_tree
            );

        let shadow_map_location = self.shadow_flow.calculate_shadow_maps(CalculationArgs
        {
            visible_sections_light: &visible_sections_light,
            ecs: render_args.ecs,
            tree: render_args.bounding_box_tree,
            camera: render_args.camera,
            visible_direction_lights: &self.visible_direction_lights,
            visible_point_lights: &self.visible_point_lights,
            visible_spot_lights: &self.visible_spot_lights
        });

        if self.enable_shadow_rendering
        {
            if let ShadowMapLocation::NewMapRequired(light_camera, light_visible_world, texture_array_index) = shadow_map_location
            {
                let upload_models = if render_args.model_bank_owner.write().any_models_changed_shadow_perspective()
                {
                    render_args.model_bank_owner.write().clear_shadow_render_system_upload_flag();
                    Some(0..self.render_systems.len() - 1)
                }
                else
                {
                    None
                };

                self.shadow_fbo.bind_fbo(BindingTarget::DrawFrameBuffer);
                self.shadow_fbo.setup_attachment(AttachmentFormat::DepthAttachment, texture_array_index as i32);
                unsafe
                    {
                        gl::Clear(gl::DEPTH_BUFFER_BIT);
                        gl::Viewport(0, 0, 1024, 1024);
                    }

                self.render_systems.last_mut().unwrap().use_vao();

                let render_args = RenderArguments
                {
                    visible_world_sections: light_visible_world,
                    bounding_box_tree: render_args.bounding_box_tree,
                    ecs: render_args.ecs,
                    camera: &light_camera,
                    model_bank_owner: render_args.model_bank_owner.clone(),
                    input_history: render_args.input_history
                };

                self.run_render_system(upload_models, self.get_shadow_render_system_index(), &render_args, &visible_sections_light);
                unsafe
                    {
                        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
                        gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
                        gl::Viewport(0, 0, self.window_dimensions.0, self.window_dimensions.1);
                    }
            }
        }

        unsafe
            {
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            }

        for index in 0..self.get_shadow_render_system_index()
        {
            self.render_systems[index].use_shader_program();
            self.render_systems[index].use_vao();

            if render_args.model_bank_owner.read().require_reupload_models_user_render_system(index)
            {
                let upload_models = Some(index..index + 1);
                self.run_render_system(upload_models, index, &render_args, &visible_sections_light);
            }
            else
            {
                let upload_models = None;
                self.run_render_system(upload_models, index, &render_args, &visible_sections_light);
            }

            render_args.model_bank_owner.write().clear_user_render_system_upload_flag(index);
        }
    }

    /// Updates the viewport to correspond with the new size of the rendering window
    ///
    /// `window_dimensions` - the resolution of the rendering window being rendered to
    pub fn update_window_dimension(&mut self, window_dimensions: (i32, i32))
    {
        unsafe{ gl::Viewport(0, 0, window_dimensions.0, window_dimensions.1); }
        self.window_dimensions = window_dimensions;
    }

    /// Renders the visible scene with the provided render system
    ///
    /// `upload_models` - the indexes of render systems whose associated models should be uploaded to
    ///                   the current render system being executed
    /// `render_system_index` - the index of the render system to run
    /// `render_args` - structure containing required variables for rendering
    fn run_render_system(&mut self, upload_models: Option<Range<usize>>, render_system_index: usize, render_args: &RenderArguments, visible_sections_light: &CullResult)
    {
        let mut models_updated = false;

        // New model or existing model is no longer used, time to reupload models into render system
        if let Some(model_bank_indexes) = upload_models
        {
            // Create copies to move into new thread where models will be uploaded into render system buffers
            let model_layout_indexes = self.render_systems[render_system_index].get_model_layout_indexes();
            let model_layout_update_function = self.render_systems[render_system_index].get_model_layout_update_function();
            let tx = self.tx.clone();

            let model_buffers = self.render_systems[render_system_index].get_model_mapped_buffers();
            let indice_buffer = self.render_systems[render_system_index].get_indice_mapped_buffer();

            let model_bank_owner = render_args.model_bank_owner.read();
            let render_system_model_banks =
                {
                    let mut model_banks = Vec::new();
                    for index in model_bank_indexes
                    {
                        model_banks.push(model_bank_owner.get_model_bank(RenderSystemIndex{ index }))
                    }

                    model_banks
                };
            RenderFlow::upload_models(&render_system_model_banks, model_buffers, indice_buffer, model_layout_update_function, model_layout_indexes, tx);

            models_updated = true;
        }

        // If there is no layout update functions, don't spend time going through the logic of updating
        // mapped buffers
        if let Some(layout_update_fn) = self.render_systems[render_system_index].get_instance_layout_update_function()
        {
            let num_unique_layouts = self.render_systems[render_system_index].get_instance_layout_indexes().len();

            let sorting_param = SortWorldSectionEntitiesParam
            {
                visible_world_sections: &render_args.visible_world_sections,
                ecs: render_args.ecs,
                bounding_box_tree: render_args.bounding_box_tree,
                unique_layout_indexes: Arc::new(self.render_systems[render_system_index].get_instance_layout_indexes()),
                layout_update_function: layout_update_fn,
                camera_position: render_args.camera.get_position(),
                draw_distance: render_args.camera.get_far_draw_distance(),
                level_views: &self.render_systems[render_system_index].level_of_views
            };

            let static_data = RenderFlow::extract_static_data(&sorting_param, self.static_data_unique_section.clone(), render_system_index);
            let sorted_data = RenderFlow::sort_world_section_active_entities(sorting_param);

            {
                let mut sorted_data = sorted_data.lock();
                let static_data = static_data.lock();
                RenderFlow::append_written_information(&mut sorted_data, &static_data, None, num_unique_layouts);
            }

            RenderFlow::upload_instance_data_to_render_system(&mut self.render_systems[render_system_index], &sorted_data.lock());
        }

        if models_updated
        {
            if let Ok(updated_model_info) = self.rx.recv()
            {
                self.render_systems[render_system_index].flush_per_model_buffers(updated_model_info.flush_info);
                self.render_systems[render_system_index].flush_indice_buffer(updated_model_info.indice_flush_info);

                // Have to synchronize the rendering information. The rendering information received
                // from the thread that uploaded the model data contains offsets for the model geometry,
                // while the rendering system rendering info has the correct instance range information

                self.render_systems[render_system_index].update_model_rendering_info(updated_model_info.updated_rendering_info);
            }
        }

        let matrices = self.shadow_flow.upload_matrices.iter().map(|x| *x).collect::<Vec<TMat4<f32>>>();
        let indexes = self.shadow_flow.upload_indexes.iter().map(|x| *x).collect::<Vec<u32>>();
        let view_matrices = self.shadow_flow.upload_view_matrices.iter().map(|x| *x).collect::<Vec<TMat4<f32>>>();

        let draw_param = DrawPreparationParameters
        {
            visible_sections_light: &visible_sections_light.visible_sections_map,
            shadow_fbo: &mut self.shadow_fbo,
            logical_entity_lookup: &HashMap::new(), // Deal with this later; have to be set in logical flow
            logical_ecs: &render_args.ecs,
            camera: render_args.camera,
            input_history: render_args.input_history,
            tree: render_args.bounding_box_tree,

            visible_directional_lights: &mut self.visible_direction_lights,
            visible_point_lights: &mut self.visible_point_lights,
            visible_spot_lights: &mut self.visible_spot_lights,
            upload_matrices: &matrices,
            upload_indexes: &indexes,
            upload_view_matrices: &view_matrices
        };

        self.render_systems[render_system_index].draw(draw_param);
    }

    /// Accumulates all static entity rendering data into one data structure to be uploaded into vRAM
    ///
    /// `sorting_param` - variables required to sort entity rendering data
    /// `static_data` - structure holding static entity data
    /// `render_system_index` - index of the render system static data is being uploaded to
    fn extract_static_data(sorting_param: &SortWorldSectionEntitiesParam, static_data: Arc<RwLock<Vec<UniqueSectionData>>>, render_system_index: usize) -> Arc<Mutex<SortResult>>
    {
        // If static entities changed in any of the visible world sections, then that data must be reloaded
        RenderFlow::sort_world_section_static_entities(sorting_param, &mut static_data.write()[render_system_index]);

        let aggregated_sorted_data: Arc<Mutex<SortResult>> = Arc::new(Mutex::new(HashMap::default()));
        let static_data_clone = static_data.clone();
        let num_unique_layouts = sorting_param.unique_layout_indexes.len();

        let extract_fn = |chunks: &[UniqueWorldSectionId]|
            {
                let mut local_static_data: SortResult = HashMap::default();

                for world_section in chunks
                {
                    if let Some(write_info) = static_data_clone.read()[render_system_index].world_data.get(world_section)
                    {
                        let distance_from_aabb = if let Some(unique_section) = sorting_param.bounding_box_tree.stored_entities_indexes.get(world_section)
                        {
                            distance_to_aabb(&unique_section.aabb, sorting_param.camera_position)
                        }
                        else
                        {
                            // This branch indicates there are static entities in a world section, but
                            // that world section does not exist
                            eprintln!("Failed to find world section: {:?}", *world_section);

                            debug_assert!(false);
                            0.0
                        };

                        if distance_from_aabb > sorting_param.draw_distance
                        {
                            continue;
                        }

                        let mut translated_model_ids = HashMap::default();

                        for (model_id, _) in write_info
                        {
                            let adjusted_model_id = match sorting_param.level_views.custom.get(&model_id)
                            {
                                Some(i) => ModelId::level_of_view_adjusted_model_index(*model_id, distance_from_aabb, i),
                                None => ModelId::level_of_view_adjusted_model_index(*model_id, distance_from_aabb, &sorting_param.level_views.default),
                            };

                            translated_model_ids.insert(*model_id, adjusted_model_id);
                        }

                        RenderFlow::append_written_information(&mut local_static_data, write_info, Some(translated_model_ids), num_unique_layouts);
                    }
                }

                let mut lock = aggregated_sorted_data.lock();
                RenderFlow::append_written_information(&mut lock, &local_static_data, None, num_unique_layouts);
            };

        if sorting_param.visible_world_sections.visible_sections_vec.is_empty()
        {
            return aggregated_sorted_data;
        }

        if cfg!(debug_assertions)
        {
            let chunk_size = sorting_param.visible_world_sections.visible_sections_vec.len();

            let _ = sorting_param.visible_world_sections.visible_sections_vec.chunks(chunk_size)
                .map(|x|
                    {
                        extract_fn(x);
                    }).collect::<()>();
        }
        else
        {
            let chunk_size = 25;

            let _ = sorting_param.visible_world_sections.visible_sections_vec.par_chunks(chunk_size).map(|x|
                {
                    extract_fn(x);
                }).collect::<()>();
        };


        aggregated_sorted_data
    }

    /// Finds any world sections where rendering information for static entities are out of data and
    /// stores that data again. Prevents unneeded checks to the ECS in the future
    ///
    /// `sorting_param` - variables required to sort entity data
    /// `unique_sections` - the data structure holding information for static entities
    fn sort_world_section_static_entities(sorting_param: &SortWorldSectionEntitiesParam, unique_sections: &mut UniqueSectionData)
    {
        if sorting_param.bounding_box_tree.get_changed_static_unique().is_empty()
        {
            return;
        }

        let processed_world_sections = Mutex::new(HashSet::default());

        let reupload_unique_world_sections = unique_sections.world_sections.intersection(sorting_param.bounding_box_tree.get_changed_static_unique())
            .map(|x| *x)
            .collect::<HashSet::<UniqueWorldSectionId>>();
        let new_upload_unique_world_sections = sorting_param.bounding_box_tree.get_changed_static_unique()
            .iter()
            .filter_map(|x| if !unique_sections.world_sections.contains(x)
            {
                Some(*x)
            }
            else
            {
                None
            })
            .collect::<HashSet::<UniqueWorldSectionId>>();

        for x in reupload_unique_world_sections.into_iter().chain(new_upload_unique_world_sections.into_iter())
        {
            let mut local_sorted_data = HashMap::default();

            let sort_world_chunk_args = SortWorldChunkArgs
            {
                world_sections: &[x],
                sorting_param: &sorting_param,
                processed_world_sections: &processed_world_sections
            };

            if let Some(shared_sections) = RenderFlow::sort_unique_world_sections(&sort_world_chunk_args, &mut local_sorted_data, &x, true)
            {
                for x in shared_sections
                {
                    RenderFlow::sort_shared_world_sections(&sort_world_chunk_args, &mut local_sorted_data, x, true);
                }
            }

            unique_sections.world_data.insert(x, local_sorted_data);
        }
    }

    /// Sorts the entities in the world section such that the returned data groups all entities that are of the same
    /// type together
    ///
    /// For example, if entity 1 and 2 represent the same model, but are in different world sections, this function
    /// will place entity 1 and 2 instance information beside each other so that they can be drawn with one draw call
    ///
    /// `sorting_param` - structure holding required variables for sorting the entities
    fn sort_world_section_active_entities(sorting_param: SortWorldSectionEntitiesParam) -> Arc<Mutex<HashMap<ModelId, HashMap<SortableIndex, WrittenInformation>>>>
    {
        let sorted_data: Arc<Mutex<HashMap<ModelId, HashMap<usize, WrittenInformation>>>> = Arc::new(Mutex::new(HashMap::default()));
        let processed_world_sections = Mutex::new(HashSet::default());

        let sort_fn = |current_world_section_chunk: &[UniqueWorldSectionId]|
            {
                let sort_world_chunk_args = SortWorldChunkArgs
                {
                    world_sections: current_world_section_chunk,
                    sorting_param: &sorting_param,
                    processed_world_sections: &processed_world_sections
                };

                let local_sorted_data = RenderFlow::sort_world_chunk(sort_world_chunk_args);

                // Time to append the sorted model layout data to the global equivalent
                let mut global_sorted_data = sorted_data.lock();

                RenderFlow::append_written_information(&mut global_sorted_data, &local_sorted_data, None, sorting_param.unique_layout_indexes.len());
            };

        let mut active_world_sections = Vec::new();
        for x in &sorting_param.visible_world_sections.visible_sections_vec
        {
            if sorting_param.bounding_box_tree.is_section_active(*x)
            {
                active_world_sections.push(*x);
            }
        }

        if active_world_sections.is_empty()
        {
            return sorted_data;
        }

        if cfg!(debug_assertions)
        {
            // In debug mode parallel implementation is very slow- sequentially is faster
            for x in active_world_sections.chunks(active_world_sections.len())
            {
                sort_fn(x);
            }
        }
        else
        {
            TimeTakeHistory::apply_to_function(&mut *VISIBLE_WORLD_SECTIONS_HISTORY.lock(), sort_fn, &active_world_sections);
        }

        sorted_data
    }

    /// Adds the information in the source to the target, effectively combining the rendering information
    ///
    /// `target` - the destination for all rendering data
    /// `source` - data to add to the target
    /// `model_id_translation` - optional translation of a source model id to a model id to store in the target
    /// `num_unique_layout` - the number of layout data stored in the WrittenInformation in target and source
    fn append_written_information(target: &mut SortResult, source: &SortResult, model_id_translation: Option<HashMap<ModelId, ModelId>>, num_unique_layouts: usize)
    {
        for (model_id, local_model_data) in source
        {
            let model_id = if let Some(ref translation) = model_id_translation
            {
                if let Some(adjusted_model_id) = translation.get(model_id)
                {
                    *adjusted_model_id
                }
                else
                {
                    // Adjusted model was never calculated. This should have been done before
                    // calling this function. Worst case default to full model detail
                    debug_assert!(false);
                    *model_id
                }
            }
            else
            {
                *model_id
            };

            match target.get_mut(&model_id)
            {
                Some(i) =>
                    {
                        for (sortable_component_index, data) in local_model_data
                        {
                            match i.get_mut(&sortable_component_index)
                            {
                                Some(j) =>
                                    {
                                        // Index of instanced layout vector data is the same in the global sorted data map
                                        //  as it is in the local sorted data map
                                        for index in 0..num_unique_layouts
                                        {
                                            j.layout_data[index].1.extend_from_slice(&data.layout_data[index].1);
                                        }

                                        j.number_entities += data.number_entities;
                                    },
                                None =>
                                    {
                                        i.insert(*sortable_component_index, data.clone());
                                    }
                            }
                        }
                    },
                None => { target.insert(model_id, local_model_data.clone()); },
            }
        }
    }

    /// Finds the data required to write to the vRAM for each model type in the given world sections
    ///
    /// `args` - structure holding variables required to perform the sorting
    fn sort_world_chunk(args: SortWorldChunkArgs) -> SortResult
    {
        let mut local_sorted_data = HashMap::default();

        for world_section in args.world_sections
        {
            // Unique sections are "keys" to shared world sections, so shared world sections are only
            // processed if the current unique world section lead to any shared sections
            if let Some(shared_sections) = RenderFlow::sort_unique_world_sections(&args, &mut local_sorted_data, world_section, false)
            {
                for x in shared_sections
                {
                    RenderFlow::sort_shared_world_sections(&args, &mut local_sorted_data, x, false);
                }
            }
        }

        local_sorted_data
    }

    /// Sorts the data required to upload to vRAM for unique world sections
    ///
    /// `args` - variables required to perform the sorting
    /// `local_sorted_data` - variable to store the result of the sorting
    /// `world_section` - the unique world section to sort entity data for
    /// `is_static` - true if the sorting is being done for static entities
    fn sort_unique_world_sections<'a>(args: &'a SortWorldChunkArgs, local_sorted_data: &mut SortResult, world_section: &UniqueWorldSectionId, is_static: bool) -> Option<&'a HashSet::<SharedWorldSectionId>>
    {
        // There are entities to process in the current world section
        if let Some(all_section_entities) = args.sorting_param.bounding_box_tree.stored_entities_indexes.get(world_section)
        {
            let distance_from_aabb = distance_to_aabb(&all_section_entities.aabb, args.sorting_param.camera_position);

            // Depending on entities in unique world section, enclosing AABB may not be that of
            // unique world section but rather the combination of all of the entities' AABBs.
            // The check to see if entities are visible may as a result not be accurate
            if distance_from_aabb < args.sorting_param.draw_distance
            {
                // Sortable indexes are iterated as entities, even if the same models, are added to separate data vectors
                // (which are uploaded to VBOs) so that information of where each model with a different sortable index is
                // stored in memory, as models with different sortable indexes can be selectively rendered. To do requires
                // passing in the sortable index of the model to add_entities
                for (index, sortable_entities) in args.sorting_param.ecs.get_entities_with_sortable().iter().enumerate()
                {
                    if is_static
                    {
                        let entities_with_sortable = all_section_entities.static_entities.intersection(sortable_entities).into_iter().map(|x| *x).collect();

                        let entity_add_args = AddEntitiesArgs
                        {
                            entities: &entities_with_sortable,
                            sorting_param: args.sorting_param,
                            local_sorted_data,
                            distance_sphere: distance_from_aabb,
                            sortable_index: index
                        };

                        RenderFlow::add_entities(entity_add_args, is_static);
                    }
                    else
                    {
                        // Intersection results in entities in the current world section that have the given sortable component
                        let entities_with_sortable = all_section_entities.local_entities.intersection(sortable_entities).into_iter().map(|x| *x).collect();

                        let entity_add_args = AddEntitiesArgs
                        {
                            entities: &entities_with_sortable,
                            sorting_param: args.sorting_param,
                            local_sorted_data,
                            distance_sphere: distance_from_aabb,
                            sortable_index: index
                        };

                        RenderFlow::add_entities(entity_add_args, is_static);
                    }
                }
            }

            return Some(&all_section_entities.shared_sections_ids);
        }

        None
    }

    /// Sorts the data required to upload to vRAM for shared world sections. This searched both for
    /// static and active entities
    ///
    /// `args` - variables required to perform the sorting
    /// `local_sorted_data` - variable to store the result of the sorting
    /// `shared_world_section_index` - the shared world section to sort entity data for
    fn sort_shared_world_sections(args: &SortWorldChunkArgs, local_sorted_data: &mut SortResult, shared_world_section_index: &SharedWorldSectionId, is_static: bool)
    {
        // True if the value was not in map when inserting
        if args.processed_world_sections.lock().insert(*shared_world_section_index)
        {
            match args.sorting_param.bounding_box_tree.shared_section_indexes.get(shared_world_section_index)
            {
                Some(i) =>
                    {
                        let distance_from_aabb = distance_to_aabb(&i.aabb, args.sorting_param.camera_position);

                        // Shared sections can extend past a unique world section away from the camera.
                        // Thus even if a unique world section that links to the shared section is visible,
                        // it does not mean that the shared section is
                        if distance_from_aabb < args.sorting_param.draw_distance
                        {
                            // Same logic as local entities
                            for (index, sortable_entities) in args.sorting_param.ecs.get_entities_with_sortable().iter().enumerate()
                            {
                                if is_static
                                {
                                    let entities_with_sortable = i.static_entities.intersection(sortable_entities).into_iter().map(|x| *x).collect();

                                    let entity_add_args = AddEntitiesArgs
                                    {
                                        entities: &entities_with_sortable,
                                        sorting_param: args.sorting_param,
                                        local_sorted_data,
                                        distance_sphere: distance_from_aabb,
                                        sortable_index: index
                                    };

                                    RenderFlow::add_entities(entity_add_args, is_static);
                                }
                                else
                                {
                                    let entities_with_sortable = i.entities.intersection(sortable_entities).into_iter().map(|x| *x).collect();

                                    let entity_add_args = AddEntitiesArgs
                                    {
                                        entities: &entities_with_sortable,
                                        sorting_param: args.sorting_param,
                                        local_sorted_data,
                                        distance_sphere: distance_from_aabb,
                                        sortable_index: index
                                    };

                                    RenderFlow::add_entities(entity_add_args, is_static);
                                }
                            }
                        }
                    },
                // This is a property of the bounding tree- a world section only points to
                // a shared section when that shared section exists- if all entities in that
                // shared section are removed, the world section no longer points to it
                None => unreachable!()
            }
        }
    }

    /// Takes the entities provided and extracts their required data to be rendered
    ///
    /// `args` - variables to extract rendering data
    /// `is_static` - true is static entities were provided to this function
    fn add_entities(args: AddEntitiesArgs, is_static: bool)
    {
        for entity in args.entities
        {
            let model_id = args.sorting_param.ecs.get_copy::<ModelId>(*entity).unwrap();
            let adjusted_model_id = if is_static
            {
                // Static entities are only uploaded once, so they should always have the base model id.
                // When they are uploaded into vRAM, an appropriate model id will be decided
                model_id
            }
            else
            {
                // Even if entities are of the same type, their geometric representation will change
                // depending on how far away they are from the user. From a rendering perspective, they
                // are effectively different models

                match args.sorting_param.level_views.custom.get(&model_id)
                {
                    Some(i) => ModelId::level_of_view_adjusted_model_index(model_id, args.distance_sphere, i),
                    None => ModelId::level_of_view_adjusted_model_index(model_id, args.distance_sphere, &args.sorting_param.level_views.default),
                }
            };

            let model_map = args.local_sorted_data.entry(adjusted_model_id).or_insert(HashMap::default());

            let written_information = match model_map.get_mut(&args.sortable_index)
            {
                Some(i) => i,
                None =>
                    {
                        let mut empty_written_information = WrittenInformation
                        {
                            number_entities: 0,
                            layout_data: Vec::new()
                        };

                        for layout_index in args.sorting_param.unique_layout_indexes.iter()
                        {
                            empty_written_information.layout_data.push((*layout_index, Vec::new()));
                        }

                        // Different entry for each sortable index, even if same model, allows for
                        // conditional rendering based off of sortable component by keeping track of
                        // where entities that have a sortable index are stored in memory
                        model_map.entry(args.sortable_index).or_insert(empty_written_information)
                    }
            };

            written_information.number_entities += 1;

            for (index, layout_index) in args.sorting_param.unique_layout_indexes.iter().enumerate()
            {
                // The index of a layout vector is NOT the same as the layout_index (since layout_indexes include
                // non-instanced layouts). The index is the index into the vector of layouts that are instanced

                // This will append the current entity's instance information to the layout vector
                let layout_vec = &mut written_information.layout_data[index].1;
                (args.sorting_param.layout_update_function)(*layout_index, &args.sorting_param.ecs, layout_vec, *entity);
            }
        }
    }

    /// Uploads the sorted world section entities into the appropriate buffers in the render system
    ///
    /// `render_system` - the render system to upload data to
    /// `data_to_write` - the instance data for the visible models to upload to the given render system
    fn upload_instance_data_to_render_system(render_system: &mut RenderSystem, data_to_write: &HashMap<ModelId, HashMap<SortableIndex, WrittenInformation>>)
    {
        // Location and associate information to write data to
        let mapped_instance_buffers = render_system.get_instanced_mapped_buffers();
        let instance_layouts = render_system.get_instance_layout_indexes();

        // Keep track of where previous model data was written so that it isn't overwritten
        let starting_byte_offset = 0;
        let mut buffer_bytes_written = vec![starting_byte_offset; mapped_instance_buffers.len()];

        let mut total_entities_processed = 0;

        for model_rendering_info in &mut render_system.model_rendering_information
        {
            for (_, instance_range) in &mut model_rendering_info.1.instance_location
            {
                // Render system will still have information about what to render from last frame.
                // The instance count controls if anything will be rendered, so it must be changed.
                // The other contents (model geometry and instance offset) doesn't matter if no
                // instances are rendered
                instance_range.count = 0;
            }
        }

        // Upload data for instances of a single model into render system
        for (model_id, layout_info) in data_to_write.iter()
        {
            let rendering_info = render_system.model_rendering_information.entry(*model_id).or_insert(ModelRenderingInformation::new());

            for (sortable_component_index, data) in layout_info
            {
                for (layout_index, layout_data) in data.layout_data.iter()
                {
                    let buffer_index = instance_layouts.iter().position(|x| x == layout_index).unwrap();

                    buffer_bytes_written[buffer_index] +=
                        MappedBuffer::write_data_serialized(mapped_instance_buffers[buffer_index], layout_data, buffer_bytes_written[buffer_index], false);
                }

                let instance_range = InstanceRange{ begin_instance: total_entities_processed, count: data.number_entities };
                rendering_info.instance_location.insert(*sortable_component_index, instance_range);

                total_entities_processed += data.number_entities;
            }
        }

        // Tell GPU to that new data in buffers is available
        let mut flush_data_request = Vec::new();
        for x in buffer_bytes_written
        {
            flush_data_request.push((0, x));
        }
        render_system.flush_per_instance_buffers(flush_data_request);
    }

    /// Uploads the model data into the appropriate buffers for the current render system
    ///
    /// `model_banks` - structure that stores the model geometry that needs to be uploaded into the render system buffers.
    ///                 This is a vector as a render system may require models that are associated with other render systems,
    ///                 such as the shadow render system
    /// `model_buffers` - pointers to model geometry buffers that data can be written into
    /// `indice_buffer` - the pointer to the indice buffer that data can be written into
    /// `model_update_fn` - function that specifies what buffer stores an aspect of the model geometry. Must be obtained from
    ///                     the current render system
    /// `model_layout_indexes` - the indexes of the layout in the shader program to use that are used for model geometry data
    /// `tx` - transmitter that can be used to send the results of uploading buffers to synchronize result with the current render system
    fn upload_models(model_banks: &Vec<&ModelBank>, model_buffers: Vec<BufferWriteInfo>, indice_buffer: BufferWriteInfo,
                     model_update_fn: ModelUpdateFunction, model_layout_indexes: Vec<u32>, tx: SyncSender<UpdateModelInfo>)
    {
        let mut number_indices_uploaded = 0;
        let mut number_vertices_uploaded = 0;

        let mut model_rendering_information = HashMap::default();

        // Vector of (StartingBufferIndex, NumberBytesChanged). An index into this buffer represents
        // an index into the layouts used for model geometry. For example, index 0 can be for vertices,
        // where as index 1 can be for the texture coordinates
        let mut layout_vector_offsets = vec![(0, 0); model_layout_indexes.len()];
        let mut indice_offset = (0, 0);

        for model_bank in model_banks
        {
            for (model_id, model_information) in model_bank.stored_models()
            {
                // Upload the model geometry into the buffers, and keep track of how many bytes were written
                // in each buffer so that next model data uploaded does not overwrite previous data
                for (index, layout_index) in model_layout_indexes.iter().enumerate()
                {
                    for mesh in &model_information.geometry.meshes
                    {
                        layout_vector_offsets[index].1 += model_update_fn(*layout_index, &mesh, model_buffers[index], layout_vector_offsets[index].1);
                    }
                }

                if !model_rendering_information.contains_key(model_id)
                {
                    // This function cannot fill in the data required for the instance information- this
                    // has to be synchronized with the render system after this function finishes
                    model_rendering_information.insert(*model_id, ModelRenderingInformation::new());
                }

                for mesh in &model_information.geometry.meshes
                {
                    let mut mesh_rendering_info = MeshRenderingInformation::new();

                    MappedBuffer::write_data_serialized(indice_buffer, &mesh.indices, (number_indices_uploaded * size_of::<u32>()) as isize, false);
                    indice_offset.1 += (mesh.indices.len() * size_of::<u32>()) as isize;

                    // Update offsets for next model data, to prevent overwriting current loop's data written
                    mesh_rendering_info.indice_offset = number_indices_uploaded;
                    mesh_rendering_info.vertex_offset = number_vertices_uploaded;
                    mesh_rendering_info.indice_count = mesh.indices.len() as i32;

                    number_indices_uploaded += mesh.indices.len();
                    number_vertices_uploaded += mesh.vertices.len() as i32;

                    model_rendering_information.get_mut(&model_id).unwrap().mesh_render_info.push(mesh_rendering_info);
                }
            }
        }

        tx.send(UpdateModelInfo{ flush_info: layout_vector_offsets, updated_rendering_info: model_rendering_information, indice_flush_info: indice_offset })
            .unwrap_or_else(|err| panic!("Failed to send model upload information: {}", err));
    }

    /// Registers the given model id with the given model name, allowing the model to be referenced by name
    /// when rendering
    ///
    /// `model_name` - the name of the model to use when rendering
    /// `model_id` - the id of model given when it was created
    /// `custom_level_of_view` - optional level of views describing what geometrical representation to use
    ///                          for rendering the model given its distance from the camera
    pub fn register_model_with_render_system(&mut self, model_name: String, model_id: ModelId, custom_level_of_view: Option<Vec<LevelOfView>>, uses_texture: bool)
    {
        self.render_systems[model_id.render_system_index.index].register_model(model_name.clone(), model_id, custom_level_of_view.clone(), uses_texture);
        let shadow_render_system_index = self.get_shadow_render_system_index();
        self.render_systems[shadow_render_system_index].register_model(model_name, model_id, custom_level_of_view, uses_texture);
    }

    pub fn remove_model(&mut self, model_id: ModelId)
    {
        for x in &mut self.render_systems
        {
            x.remove_model(model_id);
        }
    }

    pub fn add_solid_colour_texture(&mut self, render_system_index: RenderSystemIndex, colour: TVec4<u8>) -> UploadedTextureLocation
    {
        self.render_systems[render_system_index.index].add_solid_colour_texture(colour)
    }

    /// Uploads the given texture to the given render system, making it available for use when rendering
    ///
    /// `render_system_index` - the index of the render system to upload the texture to
    /// `texture_location` - the location of the texture to upload
    pub fn add_texture(&mut self, render_system_index: RenderSystemIndex, texture_location: PathBuf) -> UploadedTextureLocation
    {
        self.render_systems[render_system_index.index].add_texture(texture_location)
    }

    /// Find the index of the shadow render system
    fn get_shadow_render_system_index(&self) -> usize
    {
        // The shadow render system is always added after the user-defined render systems have
        // been stored
        self.render_systems.len() - 1
    }

    /// Initializes a new render system for creating shadow maps
    ///
    /// `level_of_views` - the level of views to use for the shadow map render system
    fn create_shadow_render_system(level_of_views: Vec<LevelOfView>, no_light_source_cutoff: f32, default_diffuse_factor: f32, shadow_draw_fn: DrawFunction,
                                   shadow_light_draw_fn: DrawFunction, shadow_transparency_draw_function: DrawFunction) -> RenderSystem
    {
        RenderSystemBuilder::new()
            .with_constants(vec![])
            .with_vertex_shader(VertexShaderInformation
            {
                write_generated_shader: None,
                glsl_version: GLSLVersion::Core430,
                shader_source: get_asset_folder().join("shaders/shadowVertex.glsl"),
                layout_info: vec!
                [
                    LayoutInformation::new(LayoutType::Vec3Float, LayoutInstance::Divisor0(1, 69696969), LayoutUse::PerModel, "aPos"),
                    LayoutInformation::new(LayoutType::Mat4x4Float, LayoutInstance::Divisor1(1, 12121212), LayoutUse::PerInstance, "translation"),
                ],
                uniforms: vec!
                [
                    UniformBlock::new("Matrices", 4, vec!
                    [
                        Uniform::new("projectionMatrix", UniformType::Mat4x4Float),
                        Uniform::new("viewMatrix", UniformType::Mat4x4Float),
                    ])
                ],
                out_variables: vec![],
                instance_layout_update_fn: Some(shadow_instance_layout_fn), // Created at end of this file
                model_layout_update_fn: shadow_layout_update_fn, // Created at end of this file
                indice_buffers: Some(IndiceInformation::new(1, 103100)),
                textures: vec![],
                cubemaps: vec![],
            })
            .with_first_pass_fragment_shader(FragmentShaderInformation
            {
                layouts: vec![],
                out_variables: vec![],
                write_generated_shader: None,
                glsl_version: GLSLVersion::Core430,
                shader_source: get_asset_folder().join("shaders/shadowFrag.glsl"),
                uniforms: vec![],
                include_shadow_maps: false,
                include_error_textures: false,
                textures: vec!
                [
                    TextureInformation
                    {
                        sampler_name: "textureArray".to_string(),
                        number_mipmaps: 5,
                        format: TextureFormat::RGBA,
                        min_filter_options: MinFilterOptions::Linear,
                        mag_filter_options: MagFilterOptions::Linear,
                        wrap_s: TextureWrap::MirroredRepeat,
                        wrap_t: TextureWrap::MirroredRepeat,
                        width: 1280,
                        height: 720,
                        number_textures: 5,
                        border_color: None
                    }
                ],
                cubemaps: vec![]
            })
            .with_no_deferred_rendering()
            .with_draw_functions(shadow_draw_fn, shadow_light_draw_fn, shadow_transparency_draw_function)
            .with_level_of_views(level_of_views)
            .with_accessible_fbos(vec![])
            .do_not_apply_nearby_lights()
            .with_light_constraints(MaxLightConstraints::NotApplicable)
            .with_no_light_diffuse_param(no_light_source_cutoff, default_diffuse_factor)
            .build()
    }
}

// Required for the shadow render system
specify_model_geometry_layouts!(shadow_layout_update_fn,
                                0, vertices);

specify_type_ids!(shadow_instance_layout_fn,
                1, TransformationMatrix);