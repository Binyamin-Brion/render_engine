use nalgebra_glm::vec3;
use render_engine::exports::load_models::{DefaultRenderSystemArgs, MaxNumLights, RenderSystemType, UserLoadRenderSystems, UserUploadInformation};
use render_engine::exports::rendering::{DrawParam, LevelOfView, ModelDrawCommand};
use render_engine::prelude::default_render_system::instance_layout_fn;
use crate::space_logic::solar_system::skybox::create_space_skybox;

pub fn load_render_systems(upload_info: &mut UserUploadInformation, render_distance: f32)
{
    let default_render_system_args = DefaultRenderSystemArgs
    {
        draw_function,
        draw_light_function,
        draw_transparency_function: draw_transparent_objects_function,
        instance_layout_update_fn: instance_layout_fn,
        level_of_views: create_level_of_views(render_distance),
        window_resolution: (1280, 720),
        sky_boxes: vec![create_space_skybox()],
        max_count_lights: MaxNumLights
        {
            directional: 1,
            point: 1,
            spot: 2
        },
        no_light_source_cutoff: 0.2,
        default_diffuse_factor: 0.2
    };

    let render_system = UserLoadRenderSystems
    {
        render_system: RenderSystemType::Default(default_render_system_args),
        render_system_name: "default".to_string()
    };

    upload_info.render_systems.push(render_system);
}

fn draw_function(mut draw_param:  &mut DrawParam)
{
    update_common_uniforms(& mut draw_param);
    draw_param.write_uniform_value("lightSource", vec![0_u32]);
    draw_param.write_uniform_value("renderingLightSource", vec![0_u32]);
    draw_param.flush_uniform_buffer();
    draw_param.draw_model_with_sortable_index(
        vec!
        [
            ModelDrawCommand{ model_name: "asteroid", component_indexes: vec![0], render_sortable_together: false, is_program_generated: false },
            ModelDrawCommand{ model_name: "mine_producer", component_indexes: vec![0], render_sortable_together: false, is_program_generated: false },
        ]
    );

    draw_param.set_fence_uniform_buffer();
}

fn draw_light_function(mut draw_param:  &mut DrawParam)
{
    update_common_uniforms(& mut draw_param);
    draw_param.write_uniform_value("lightSource", vec![1_u32]);
    draw_param.write_uniform_value("renderingLightSource", vec![1_u32]);
    draw_param.write_vec3("skyboxBrightness", vec3(6.0_f32, 6.0, 6.0));
    draw_param.flush_uniform_buffer();
    draw_param.draw_model_with_sortable_index(
        vec!
        [
            ModelDrawCommand{ model_name: "yellowStar", component_indexes: vec![3], render_sortable_together: false, is_program_generated: false },
            ModelDrawCommand{ model_name: "blueStar", component_indexes: vec![3], render_sortable_together: false, is_program_generated: false }
        ]
    );

    draw_param.set_fence_uniform_buffer();
}

fn draw_transparent_objects_function(mut draw_param:  &mut DrawParam)
{
    update_common_uniforms(& mut draw_param);
    draw_param.write_uniform_value("lightSource", vec![0_u32]);
    draw_param.write_uniform_value("renderingLightSource", vec![0_u32]);

    unsafe
        {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }

    draw_param.draw_model_with_sortable_index(
        vec!
        [
            ModelDrawCommand{ model_name: "wormhole", component_indexes: vec![0], render_sortable_together: false, is_program_generated: false },
        ]
    );

    unsafe
        {
            gl::Disable(gl::BLEND);
        }

    draw_param.set_fence_uniform_buffer();
}

fn update_common_uniforms(draw_param: &mut DrawParam)
{
    draw_param.write_uniform_value("projectionMatrix", vec![draw_param.get_camera().get_projection_matrix()]);
    draw_param.write_uniform_value("viewMatrix", vec![draw_param.get_camera().get_view_matrix()]);
    draw_param.write_uniform_value("cameraLocation", vec![draw_param.get_camera().get_position()]);
}

fn create_level_of_views(render_distance: f32) -> Vec<LevelOfView>
{
    let first_view_distance = render_distance * 0.10;
    let second_view_distance = render_distance * 0.15 + first_view_distance;
    let third_view_distance = render_distance * 0.20 + second_view_distance;
    let fourth_view_distance = render_distance * 0.25 + third_view_distance;
    let fifth_view_distance = render_distance * 0.30 + fourth_view_distance;

    vec!
    [
        LevelOfView{ min_distance: 0.0, max_distance: first_view_distance },
        LevelOfView{ min_distance: first_view_distance, max_distance: second_view_distance },
        LevelOfView{ min_distance: second_view_distance, max_distance: third_view_distance },
        LevelOfView{ min_distance: third_view_distance, max_distance: fourth_view_distance },
        LevelOfView{ min_distance: fourth_view_distance, max_distance: fifth_view_distance }
    ]
}