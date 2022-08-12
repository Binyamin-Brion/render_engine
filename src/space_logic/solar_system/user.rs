use std::any::TypeId;
use nalgebra_glm::vec3;
use render_engine::exports::movement_components::{Acceleration, Velocity};
use render_engine::objects::ecs::{ECS, TypeIdentifier};
use render_engine::objects::entity_change_request::{EntityChangeInformation, EntityChangeRequest};
use render_engine::objects::entity_id::{EntityId, EntityIdRead};
use render_engine::world::bounding_box_tree_v2::BoundingBoxTree;
use crate::space_logic::solar_system::mine_producer::MineProducer;
use crate::space_logic::solar_system::wormhole::WormHole;

pub fn per_frame_logic(_: EntityId, _: &ECS, _: &BoundingBoxTree, _: f32) -> Vec<EntityChangeInformation>
{
    println!("in frame logic");
    vec![]
}

pub fn collision_logic(self_id: EntityId, other_id: EntityIdRead, ecs: &ECS, _: &BoundingBoxTree) -> Vec<EntityChangeInformation>
{
    println!("collision");
    if let Some(entity_type) = ecs.get_entity_type_read(other_id)
    {
        if entity_type == TypeIdentifier::from(TypeId::of::<WormHole>())
        {
            let curr_velocity = ecs.get_copy::<Velocity>(self_id).unwrap().get_velocity();

            if curr_velocity.x == 0.0
            {
                let mut modification = EntityChangeRequest::new(self_id);
                modification.add_new_change(Acceleration::new(vec3(-2.0, 0.0, -1.0)));
                modification.add_new_change(Velocity::new(vec3(75.0, 0.0, -35.0)));
                vec![
                    EntityChangeInformation::ModifyRequest(modification)
                ]
            } else { vec![] }
        }
        else if entity_type == TypeIdentifier::from(TypeId::of::<MineProducer>())
        {
            println!(">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>");
            vec![]
        }
        else
        {
            vec![]
        }
    }
    else
    {
        vec![]
    }
}