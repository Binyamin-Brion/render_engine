use std::path::PathBuf;
use hashbrown::HashMap;
use nalgebra_glm::TVec4;
use crate::exports::camera_object::Camera;
use crate::exports::logic_components::{CollisionLogic, EntityLogic, OutOfBoundsLogic, UserInputLogic};
use crate::exports::rendering::LevelOfView;
use crate::objects::ecs::{ECS, TypeIdentifier};
use crate::objects::entity_id::EntityId;
use crate::render_system::render_system::{InstancedLayoutWriteFunction, RenderSystem};
use crate::render_system::system_information::DrawFunction;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;
use crate::world::bounding_volumes::aabb::StaticAABB;

pub type AddInstanceFunction = fn(&mut ECS, Vec<EntityId>, &mut BoundingBoxTree, StaticAABB);
pub type RegisterInstancesFunction = fn(&mut ECS);

pub struct UserUploadInformation
{
    pub window_resolution: (u32, u32),
    pub max_fps: i64,
    pub world_section_length: u32,
    pub initial_camera: Camera,
    pub render_systems: Vec<UserLoadRenderSystems>,
    pub load_models: Vec<UserLoadModelInfo>,
    pub load_instances: Vec<UserLoadModelInstances>,
    pub instance_logic: InstanceLogic,
    pub shadow_render_system_lov: Option<Vec<LevelOfView>>,
    pub shadow_draw_fn: DrawFunction,
    pub shadow_transparency_draw_fn: DrawFunction,
    pub shadow_light_draw_fn: DrawFunction,
    pub is_debugging: bool,
    pub model_texture_dir: PathBuf,
    pub user_collision_function: CollisionLogic,
    pub user_logic_function: EntityLogic,
    pub user_original_aabb: StaticAABB,
    pub user_input_functions: Vec<UserInputLogic>,
    pub register_instance_function: Vec<RegisterInstancesFunction>,
}

unsafe impl Send for UserUploadInformation {}

impl UserUploadInformation
{
    pub fn new(initial_camera: Camera, shadow_draw_fn: DrawFunction, shadow_light_draw_fn: DrawFunction, shadow_transparency_draw_fn: DrawFunction,
               model_texture_dir: PathBuf, user_collision_function: CollisionLogic, user_logic_function: EntityLogic, user_original_aabb: StaticAABB,
               user_input_functions: Vec<UserInputLogic>) -> UserUploadInformation
    {
        UserUploadInformation
        {
            window_resolution: (initial_camera.window_width as u32, initial_camera.window_height as u32),
            max_fps: 60,
            world_section_length: 64,
            initial_camera,
            render_systems: vec![],
            load_models: vec![],
            load_instances: vec![],
            instance_logic: InstanceLogic::new(),
            shadow_render_system_lov: None,
            shadow_draw_fn,
            shadow_light_draw_fn,
            shadow_transparency_draw_fn,
            is_debugging: false,
            model_texture_dir,
            user_collision_function,
            user_logic_function,
            user_original_aabb,
            user_input_functions,
            register_instance_function: Vec::new()
        }
    }
}

pub struct InstanceLogic
{
    pub entity_logic: HashMap<TypeIdentifier, EntityLogic>,
    pub random_entity_logic: HashMap<TypeIdentifier, EntityLogic>,
    pub collision_logic: HashMap<TypeIdentifier, CollisionLogic>,
    pub random_collision_logic: HashMap<TypeIdentifier, CollisionLogic>,
    pub out_of_bounds_logic: HashMap<TypeIdentifier, OutOfBoundsLogic>
}

impl InstanceLogic
{
    pub fn new() -> InstanceLogic
    {
        InstanceLogic
        {
            entity_logic: HashMap::default(),
            random_entity_logic: HashMap::default(),
            collision_logic: HashMap::default(),
            random_collision_logic: HashMap::default(),
            out_of_bounds_logic: HashMap::default()
        }
    }
}

pub struct MaxNumLights
{
    pub directional: u16,
    pub point: u16,
    pub spot: u16
}

pub struct DefaultRenderSystemArgs
{
    pub draw_function: DrawFunction,
    pub draw_light_function: DrawFunction,
    pub draw_transparency_function: DrawFunction,
    pub instance_layout_update_fn: InstancedLayoutWriteFunction,
    pub level_of_views: Vec<LevelOfView>,
    pub window_resolution: (i32, i32),
    pub sky_boxes: Vec<UserLoadSkyBoxModels>,
    pub max_count_lights: MaxNumLights,
    pub no_light_source_cutoff: f32,
    pub default_diffuse_factor: f32,
}

pub enum RenderSystemType
{
    Default(DefaultRenderSystemArgs),
    Custom(RenderSystem),
}

pub struct UserLoadRenderSystems
{
    pub render_system: RenderSystemType,
    pub render_system_name: String,
}

pub struct UserLevelOfView
{
    pub min: f32,
    pub max: f32,
}

pub struct UserLoadModelInfo
{
    pub model_name: String,
    pub render_system_index: String,
    pub location: Vec<PathBuf>,
    pub custom_level_of_view: Option<Vec<UserLevelOfView>>,
    pub solid_colour_texture: Option<TVec4<u8>>,
}

pub struct UserLoadModelInstances
{
    pub model_name: String,
    pub num_instances: usize,
    pub upload_fn: AddInstanceFunction,
}

pub struct UserLoadSkyBoxModels
{
    pub sky_box_name: String,
    pub textures: Vec<PathBuf>
}