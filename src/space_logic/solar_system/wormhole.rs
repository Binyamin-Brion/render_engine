use std::any::TypeId;
use nalgebra_glm::{vec3, vec4};
use render_engine::exports::entity_transformer::EntityTransformationBuilder;
use render_engine::exports::load_models::{UserLoadModelInfo, UserLoadModelInstances, UserUploadInformation};
use render_engine::exports::movement_components::{Position, Scale};
use render_engine::objects::ecs::{ECS, TypeIdentifier};
use render_engine::objects::entity_change_request::EntityChangeInformation;
use render_engine::objects::entity_id::EntityId;
use render_engine::world::bounding_box_tree_v2::BoundingBoxTree;
use render_engine::world::bounding_volumes::aabb::StaticAABB;
use crate::space_logic::helper_functionality::directory_lookup::get_model_dir;

pub struct WormHole;

fn _asteroid_logic(_: EntityId, _: &ECS, _tree: &BoundingBoxTree, _time: f32) -> Vec<EntityChangeInformation>
{
    vec![]
}

pub fn create_wormhole(upload_info: &mut UserUploadInformation)
{
    load_wormhole(upload_info);
    load_wormhole_instances(upload_info);
}

fn load_wormhole(upload_info: &mut UserUploadInformation)
{
    let get_wormhole_model = ||
        {
            get_model_dir().join("wormhole.obj")
        };

    let wormhole_model = UserLoadModelInfo
    {
        model_name: "wormhole".to_string(),
        render_system_index: "default".to_string(),
        location: vec!
        [
            get_wormhole_model(),
            get_wormhole_model(),
            get_wormhole_model(),
            get_wormhole_model(),
            get_wormhole_model()
        ],
        custom_level_of_view: None,
        solid_colour_texture: Some(vec4(230, 87, 230, 64))
    };

    upload_info.load_models.push(wormhole_model);
}

fn load_wormhole_instances(upload_info: &mut UserUploadInformation)
{
    let instance_info = UserLoadModelInstances
    {
        model_name: "wormhole".to_string(),
        num_instances: 1,
        upload_fn: load_wormhole_instance_helper
    };

    upload_info.load_instances.push(instance_info);
}

fn load_wormhole_instance_helper(ecs: &mut ECS, created_entities: Vec<EntityId>, bounding_tree: &mut BoundingBoxTree,  aabb: StaticAABB)
{
    for entity in created_entities
    {
        EntityTransformationBuilder::new(entity, false, None, false)
            .with_translation(Position::new(vec3(970.0, 1000.0, 1000.0)))
            .with_scale(Scale::new(vec3(5.0, 5.00, 5.00)))
            .apply_choices(aabb, ecs, bounding_tree);

        ecs.write_entity_type(entity, TypeIdentifier::from(TypeId::of::<WormHole>()));
    }
}