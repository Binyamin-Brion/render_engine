use nalgebra_glm::vec3;
use render_engine::exports::camera_object::CameraBuilder;
use render_engine::exports::load_models::UserUploadInformation;
use render_engine::exports::logic_components::{CollisionLogic, EntityLogic};
use render_engine::exports::rendering::DrawParam;
use render_engine::launch_render_system;
use render_engine::world::bounding_volumes::aabb::StaticAABB;
use render_engine::world::dimension::range::{XRange, YRange, ZRange};
use crate::space_logic::helper_functionality::directory_lookup::get_model_texture_dir;
use crate::space_logic::render_systems::render_system_setup::load_render_systems;
use crate::space_logic::solar_system::asteroid::create_asteroid;
use crate::space_logic::solar_system::mine_producer::create_mine_producer;
use crate::space_logic::solar_system::sun::create_star;
use crate::space_logic::solar_system::user::{collision_logic, per_frame_logic};
use crate::space_logic::solar_system::wormhole::create_wormhole;
use crate::space_logic::user_input::create_user_logic;

mod space_logic;

fn main()
{
    let window_dimensions = (1280, 720);
    let draw_distance = 1000.0;

    let camera = CameraBuilder::new(window_dimensions)
        .with_position(vec3(1000.0, 1000.0, 1150.0))
        .with_direction(vec3(0.0, 0.0, -1.0))
        .with_yaw(-90.0)
        .with_far_draw_distance(draw_distance)
        .with_movement_speed_factor(60.0).build();

    let user_collision_function = CollisionLogic{ logic: collision_logic };
    let user_logic_function = EntityLogic{ logic: per_frame_logic };

    let aabb_half_size = 5.0;
    let user_aabb = StaticAABB::new
        (
            XRange::new(-aabb_half_size, aabb_half_size),
            YRange::new(-aabb_half_size, aabb_half_size),
            ZRange::new(-aabb_half_size, aabb_half_size)
        );

    let mut user_upload_information = UserUploadInformation::new(camera, shadow_draw_fn, shadow_light_draw_fn, shadow_transparency_draw_fn,
                                                                 get_model_texture_dir(), user_collision_function, user_logic_function, user_aabb,
                                                                 create_user_logic());
    user_upload_information.max_fps = 60;

    if cfg!(debug_assertions)
    {
        user_upload_information.world_section_length = 64;
    }

    user_upload_information.is_debugging = false;

    load_render_systems(&mut user_upload_information, draw_distance);
    create_star(&mut user_upload_information);
    create_asteroid(&mut user_upload_information);
    create_wormhole(&mut user_upload_information);
    create_mine_producer(&mut user_upload_information);

    launch_render_system(user_upload_information);
}

pub fn shadow_draw_fn(_draw_param: &mut DrawParam)
{
    return;
}

pub fn shadow_light_draw_fn(_draw_param: &mut DrawParam)
{
    return;
}

pub fn shadow_transparency_draw_fn(_draw_param: &mut DrawParam)
{
    return;
}