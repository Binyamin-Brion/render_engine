use std::any::TypeId;
use std::path::PathBuf;
use hashbrown::HashMap;
use nalgebra_glm::TVec3;
use crate::exports::camera_object::Camera;
use crate::exports::logic_components::{CollisionLogic, EntityLogic, OutOfBoundsLogic};
use crate::exports::rendering::LevelOfView;
use crate::objects::ecs::{ECS, TypeIdentifier};
use crate::objects::entity_id::EntityId;
use crate::render_system::render_system::{InstancedLayoutWriteFunction, RenderSystem};
use crate::render_system::system_information::DrawFunction;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;
use crate::world::bounding_volumes::aabb::StaticAABB;

pub type AddInstanceFunction = fn(&mut ECS, Vec<EntityId>, &mut BoundingBoxTree, StaticAABB);

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
    pub is_debugging: bool,
}

unsafe impl Send for UserUploadInformation {}

impl UserUploadInformation
{
    pub fn new(initial_camera: Camera, shadow_draw_fn: DrawFunction) -> UserUploadInformation
    {
        UserUploadInformation
        {
            window_resolution: (1280, 720),
            max_fps: 60,
            world_section_length: 64,
            initial_camera,
            render_systems: vec![],
            load_models: vec![],
            load_instances: vec![],
            instance_logic: InstanceLogic::new(),
            shadow_render_system_lov: None,
            shadow_draw_fn,
            is_debugging: false
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
    pub instance_layout_update_fn: InstancedLayoutWriteFunction,
    pub level_of_views: Vec<LevelOfView>,
    pub window_resolution: (i32, i32),
    pub sky_boxes: Vec<UserLoadSkyBoxModels>,
    pub max_count_lights: MaxNumLights
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