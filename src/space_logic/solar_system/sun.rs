use std::any::TypeId;
use nalgebra_glm::{vec3, vec4};
use render_engine::exports::entity_transformer::EntityTransformationBuilder;
use render_engine::exports::light_components::{FindLightType, LightInformation, SpotLight};
use render_engine::exports::load_models::{UserLoadModelInfo, UserLoadModelInstances, UserUploadInformation};
use render_engine::exports::movement_components::{Position, Rotation, Scale, VelocityRotation};
use render_engine::objects::ecs::{ECS, TypeIdentifier};
use render_engine::objects::entity_id::EntityId;
use render_engine::world::bounding_box_tree_v2::BoundingBoxTree;
use render_engine::world::bounding_volumes::aabb::StaticAABB;
use crate::space_logic::helper_functionality::directory_lookup::get_model_dir;
use crate::space_logic::solar_system::system_creator::{generate_random_name, InstanceInfo, INSTANCES};

pub fn create_star(upload_info: &mut UserUploadInformation)
{
    load_star_model(upload_info);
    load_star_instances(upload_info);
}

fn load_star_model(upload_info: &mut UserUploadInformation)
{
    let get_yellow_star_model = ||
        {
            get_model_dir().join("yellow_star.obj")
        };

    let get_blue_star_model = ||
        {
            get_model_dir().join("blue_star.obj")
        };

    let yellow_star_model = UserLoadModelInfo
    {
        model_name: "yellowStar".to_string(),
        render_system_index: "default".to_string(),
        location: vec!
        [
            get_yellow_star_model(),
            get_yellow_star_model(),
            get_yellow_star_model(),
            get_yellow_star_model(),
            get_yellow_star_model(),
        ],
        custom_level_of_view: None,
        solid_colour_texture: None,
    };

    let blue_star_model = UserLoadModelInfo
    {
        model_name: "blueStar".to_string(),
        render_system_index: "default".to_string(),
        location: vec!
        [
            get_blue_star_model(),
            get_blue_star_model(),
            get_blue_star_model(),
            get_blue_star_model(),
            get_blue_star_model(),
        ],
        custom_level_of_view: None,
        solid_colour_texture: None
    };

    upload_info.load_models.push(yellow_star_model);
    upload_info.load_models.push(blue_star_model);
}

fn load_star_instances(upload_info: &mut UserUploadInformation)
{
    let num_instances = 1;

    let yellow_star_instance_info = UserLoadModelInstances
    {
        model_name: "yellowStar".to_string(),
        num_instances,
        upload_fn: load_yellow_star_instance_helper
    };

    let blue_star_instance_info = UserLoadModelInstances
    {
        model_name: "blueStar".to_string(),
        num_instances,
        upload_fn: load_blue_star_instance_helper
    };

    let mut lock = INSTANCES.lock().unwrap();
    lock.insert("yellowStar".to_string(), InstanceInfo::new(num_instances));
    lock.insert("blueStar".to_string(), InstanceInfo::new(num_instances));

    upload_info.load_instances.push(yellow_star_instance_info);
    upload_info.load_instances.push(blue_star_instance_info);
}

fn load_yellow_star_instance_helper(ecs: &mut ECS, created_entities: Vec<EntityId>, bounding_tree: &mut BoundingBoxTree,  aabb: StaticAABB)
{
    for (index, entity) in created_entities.iter().enumerate()
    {
        EntityTransformationBuilder::new(*entity, false, Some(FindLightType::Spot), false)
            .with_translation(Position::new(vec3(950.0 + (index as f32 * 100.0), 1000.0, 965.0)))
            .with_rotation(Rotation::default())
            .with_rotation_velocity(VelocityRotation::new(vec3(0.0, 1.0, 0.0), -40_f32.to_radians()))
            .with_scale(Scale::new(vec3(10.0, 10.0, 10.0)))
            .apply_choices(aabb, ecs, bounding_tree);

        let light_information = LightInformation
        {
            radius: 500.0,
            diffuse_colour: vec3(1.0, 0.6, 0.0),
            specular_colour: vec3(1.0, 0.6, 0.0),
            ambient_colour: vec4(1.0, 0.6, 0.0, 0.25),
            linear_coefficient: 0.007,
            quadratic_coefficient: 0.0002,
            cutoff: None,
            outer_cutoff: None,
            direction: None,
            fov: None,
        };

        ecs.write_component::<LightInformation>(*entity, light_information);
        ecs.write_sortable_component(*entity, TypeIdentifier::from(TypeId::of::<SpotLight>()));

        let mut lock = INSTANCES.lock().unwrap();
        let map = lock.get_mut("yellowStar").unwrap();
        map.specific_instance.insert(generate_random_name(), *entity);
    }
}

fn load_blue_star_instance_helper(ecs: &mut ECS, created_entities: Vec<EntityId>, bounding_tree: &mut BoundingBoxTree,  aabb: StaticAABB)
{
    for (index, entity) in created_entities.iter().enumerate()
    {
        EntityTransformationBuilder::new(*entity, false, Some(FindLightType::Spot), false)
            .with_translation(Position::new(vec3(1050.0 + (index as f32 * 100.0), 1000.0, 965.0)))
            .with_rotation(Rotation::default())
            .with_rotation_velocity(VelocityRotation::new(vec3(0.0, 1.0, 0.0), 50_f32.to_radians()))
            .with_scale(Scale::new(vec3(15.0, 15.0, 15.0)))
            .apply_choices(aabb, ecs, bounding_tree);

        let light_information = LightInformation
        {
            radius: 500.0,
            diffuse_colour: vec3(0.2, 0.3, 1.0),
            specular_colour: vec3(0.2, 0.3, 1.0),
            ambient_colour: vec4(0.2, 0.3, 1.0, 0.25),
            linear_coefficient: 0.007,
            quadratic_coefficient: 0.0002,
            cutoff: None,
            outer_cutoff: None,
            direction: None,
            fov: None,
        };

        ecs.write_component::<LightInformation>(*entity, light_information);
        ecs.write_sortable_component(*entity, TypeIdentifier::from(TypeId::of::<SpotLight>()));

        let mut lock = INSTANCES.lock().unwrap();
        let map = lock.get_mut("blueStar").unwrap();
        map.specific_instance.insert(generate_random_name(), *entity);
    }
}