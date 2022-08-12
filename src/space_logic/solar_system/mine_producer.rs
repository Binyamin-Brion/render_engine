use std::any::TypeId;
use nalgebra_glm::{vec3, vec4};
use render_engine::exports::entity_transformer::EntityTransformationBuilder;
use render_engine::exports::load_models::{UserLoadModelInfo, UserLoadModelInstances, UserUploadInformation};
use render_engine::exports::logic_components::EntityLogic;
use render_engine::exports::movement_components::{Position, Rotation, Scale, VelocityRotation};
use render_engine::objects::ecs::{ECS, TypeIdentifier};
use render_engine::objects::entity_change_request::EntityChangeInformation;
use render_engine::objects::entity_id::EntityId;
use render_engine::world::bounding_box_tree_v2::BoundingBoxTree;
use render_engine::world::bounding_volumes::aabb::StaticAABB;
use crate::space_logic::helper_functionality::directory_lookup::get_model_dir;

pub struct MineProducer;

fn mine_producer_logic(this: EntityId, ecs: &ECS, _tree: &BoundingBoxTree, delta_time: f32) -> Vec<EntityChangeInformation>
{
    // TODO: Why is rotation axis always increasing?
    let rotation = ecs.get_copy::<Rotation>(this).unwrap();
    println!("Rotation is: {:?} | {}", rotation.get_rotation_axis(), rotation.get_rotation());



    vec![]
}

pub fn create_mine_producer(upload_info: &mut UserUploadInformation)
{
    upload_info.instance_logic.entity_logic.insert(TypeIdentifier::from(TypeId::of::<MineProducer>()), EntityLogic{ logic: mine_producer_logic });

    load_mine_producer(upload_info);
    load_mine_producer_instances(upload_info);
}

fn load_mine_producer(upload_info: &mut UserUploadInformation)
{
    let get_mine_producer_model = ||
        {
            get_model_dir().join("mine_producer.obj")
        };

    let mine_producer_model = UserLoadModelInfo
    {
        model_name: "mine_producer".to_string(),
        render_system_index: "default".to_string(),
        location: vec!
        [
            get_mine_producer_model(),
            get_mine_producer_model(),
            get_mine_producer_model(),
            get_mine_producer_model(),
            get_mine_producer_model()
        ],
        custom_level_of_view: None,
        solid_colour_texture: Some(vec4(200, 150, 200, 64))
    };

    upload_info.load_models.push(mine_producer_model);
}

fn load_mine_producer_instances(upload_info: &mut UserUploadInformation)
{
    let instance_info = UserLoadModelInstances
    {
        model_name: "mine_producer".to_string(),
        num_instances: 1,
        upload_fn: load_mine_producer_instance_helper
    };

    upload_info.load_instances.push(instance_info);
}

fn load_mine_producer_instance_helper(ecs: &mut ECS, created_entities: Vec<EntityId>, bounding_tree: &mut BoundingBoxTree,  aabb: StaticAABB)
{
    for entity in created_entities
    {
        EntityTransformationBuilder::new(entity, false, None, false)
            .with_translation(Position::new(vec3(980.0, 1000.0, 1000.0)))
            .with_scale(Scale::new(vec3(5.0, 5.00, 5.00)))
            .with_rotation(Rotation::new(vec3(1.0, 0.0, 0.0), 0.0))
            .with_rotation_velocity(VelocityRotation::new(vec3(1.0, 0.0, 0.0), 30.0_f32.to_radians()))
            .apply_choices(aabb, ecs, bounding_tree);

        ecs.write_entity_type(entity, TypeIdentifier::from(TypeId::of::<MineProducer>()));
    }
}