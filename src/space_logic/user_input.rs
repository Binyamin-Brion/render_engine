use glfw::Key;
use render_engine::exports::camera_object::{Camera, MovementFactor};
use render_engine::exports::logic_components::UserInputLogic;
use render_engine::objects::ecs::ECS;
use render_engine::objects::entity_change_request::{EntityChangeInformation, EntityChangeRequest};
use render_engine::objects::entity_id::EntityId;
use render_engine::window::gl_window::MIDDLE_BUTTON;
use render_engine::window::input_state::{CurrentFrameInput, InputHistory};
use render_engine::world::bounding_box_tree_v2::BoundingBoxTree;
use serde::{Serialize, Deserialize};

pub fn create_user_logic() -> Vec<UserInputLogic>
{
    vec!
    [
        UserInputLogic{ logic: move_camera }
    ]
}

pub fn move_camera(this: EntityId, ecs: &ECS, _: &BoundingBoxTree, camera: &mut Camera, input: &InputHistory, current_input: &CurrentFrameInput, elapsed_time: f32) -> Vec<EntityChangeInformation>
{

    let mut move_factor = match ecs.get_copy::<MovementFactor>(this)
    {
        Some(i) => i,
        None => MovementFactor { forwards_backwards: 0.0, left_right: 0.0 },
    };

    let max_movement_factor = 75.0;

    let time_to_completely_accelerate = 0.0000000001;
    let time_to_completely_deaccelerate = 0.0000000001;

    let per_second_movement_acceleration = max_movement_factor / time_to_completely_accelerate;
    let per_second_movement_deacceleration = max_movement_factor / time_to_completely_deaccelerate;

    let mut forward_backwards_key_down = false;
    let mut left_right_key_down = false;

    if input.is_key_down(Key::W)
    {
        move_factor.forwards_backwards = (move_factor.forwards_backwards + per_second_movement_acceleration * elapsed_time).min(max_movement_factor);
        forward_backwards_key_down = true;
    }

    if input.is_key_down(Key::S)
    {
        move_factor.forwards_backwards = (move_factor.forwards_backwards - per_second_movement_acceleration * elapsed_time).max(-max_movement_factor);
        forward_backwards_key_down = true;
    }

    if input.is_key_down(Key::D)
    {
        move_factor.left_right = (move_factor.left_right + per_second_movement_acceleration * elapsed_time).min(max_movement_factor);
        left_right_key_down = true;
    }

    if input.is_key_down(Key::A)
    {
        move_factor.left_right = (move_factor.left_right - per_second_movement_acceleration * elapsed_time).max(-max_movement_factor);
        left_right_key_down = true;
    }

    if move_factor.forwards_backwards > 0.0 && !forward_backwards_key_down
    {
        move_factor.forwards_backwards = (move_factor.forwards_backwards - per_second_movement_deacceleration * elapsed_time).max(0.0);
    }
    else if move_factor.forwards_backwards < 0.0 && !forward_backwards_key_down
    {
        move_factor.forwards_backwards = (move_factor.forwards_backwards + per_second_movement_deacceleration * elapsed_time).min(0.0);
    }

    if move_factor.left_right > 0.0 && !left_right_key_down
    {
        move_factor.left_right = (move_factor.left_right - per_second_movement_deacceleration * elapsed_time).max(0.0);
    }
    else if move_factor.left_right < 0.0 && !left_right_key_down
    {
        move_factor.left_right = (move_factor.left_right + per_second_movement_deacceleration * elapsed_time).min(0.0);
    }

    camera.float_position(move_factor, elapsed_time);

    if input.is_mouse_down(MIDDLE_BUTTON)
    {
        if let Some((new_x, new_y)) = current_input.get_latest_cursor_pos()
        {
            camera.rotate(new_x, new_y);
        }
    }
    else
    {
        camera.first_rotation = true;
    }

    let mut entity_changes = EntityChangeRequest::new(this);
    entity_changes.add_new_change(move_factor);

    vec![EntityChangeInformation::ModifyRequest(entity_changes)]
}