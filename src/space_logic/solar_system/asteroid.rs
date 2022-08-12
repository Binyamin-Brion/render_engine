use std::any::TypeId;
use nalgebra_glm::{TVec3, vec3};
use rand::{Rng, thread_rng};
use render_engine::exports::entity_transformer::EntityTransformationBuilder;
use render_engine::exports::load_models::{UserLoadModelInfo, UserLoadModelInstances, UserUploadInformation};
use render_engine::exports::movement_components::{Position, Rotation, Scale, VelocityRotation};
use render_engine::objects::ecs::{ECS, TypeIdentifier};
use render_engine::objects::entity_id::EntityId;
use render_engine::world::bounding_box_tree_v2::BoundingBoxTree;
use render_engine::world::bounding_volumes::aabb::StaticAABB;
use serde::{Deserialize, Serialize};
use crate::space_logic::helper_functionality::directory_lookup::get_model_dir;
use crate::space_logic::solar_system::system_creator::INSTANCES;

pub struct Asteroid;

const ASTERIOD_PER_SUN: usize = 20;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AngleRelativeSun
{
    radius: f32,
    offset: TVec3<f32>,
    xz_angle: f32, // 0 degree = "positive x-axis",
change_xz: f32,
    existence_time: f32,
}

pub fn create_asteroid(upload_info: &mut UserUploadInformation)
{
    load_asteroid(upload_info);
    load_asteroid_instances(upload_info);
}

fn load_asteroid(upload_info: &mut UserUploadInformation)
{
    let get_asteroid_model = ||
        {
            get_model_dir().join("asteroid2.obj")
        };

    let asteroid_model = UserLoadModelInfo
    {
        model_name: "asteroid".to_string(),
        render_system_index: "default".to_string(),
        location: vec!
        [
            get_asteroid_model(),
            get_asteroid_model(),
            get_asteroid_model(),
            get_asteroid_model(),
            get_asteroid_model(),
        ],
        custom_level_of_view: None,
        solid_colour_texture: None,
    };

    upload_info.load_models.push(asteroid_model);
}

fn load_asteroid_instances(upload_info: &mut UserUploadInformation)
{
    let mut number_suns = match INSTANCES.lock().unwrap().get("yellowStar")
    {
        Some(i) => i.get_num_instances() * ASTERIOD_PER_SUN,
        None => 0,
    };

    number_suns += match INSTANCES.lock().unwrap().get("blueStar")
    {
        Some(i) => i.get_num_instances() * ASTERIOD_PER_SUN,
        None => 0,
    };

    let instance_info = UserLoadModelInstances
    {
        model_name: "asteroid".to_string(),
        num_instances: number_suns,
        upload_fn: load_asteroid_instance_helper
    };

    upload_info.load_instances.push(instance_info);
}

fn load_asteroid_instance_helper(ecs: &mut ECS, created_entities: Vec<EntityId>, bounding_tree: &mut BoundingBoxTree,  aabb: StaticAABB)
{
    ecs.register_type::<AngleRelativeSun>();
    let mut rng = thread_rng();

    let lock = INSTANCES.lock().unwrap();

    let mut entities_processed = 0;
    let asteroids_per_sun = ASTERIOD_PER_SUN;

    if let Some(ref suns) = lock.get("yellowStar")
    {
        for (_, sun_entity) in &suns.specific_instance
        {
            for entity in created_entities.iter().skip(entities_processed).take(asteroids_per_sun)
            {
                let offset = ecs.get_copy::<Position>(*sun_entity).unwrap();

                let mut angle = AngleRelativeSun
                {
                    xz_angle: rng.gen_range(0.0..360.0),
                    radius: rng.gen_range(30.0..50.0),
                    offset: vec3(offset.get_position().x, 1000.0 + rng.gen_range(-20.00..20.0), offset.get_position().z),
                    change_xz: rng.gen_range(0.1..0.5),
                    // radius: 20.0,
                    // offset: vec3(1000.0, 1000.0, 1020.0),
                    // xz_angle: 0.0,
                    // change_xz: 0.0
                    existence_time: 0.0
                };

                let position = calculate_position(&mut angle);

                EntityTransformationBuilder::new(*entity, false, None, false)
                    .with_translation(position)
                    .with_rotation(Rotation::new(vec3(0.0, 1.0, 0.0), 0.1_f32.to_radians()))
                    .with_rotation_velocity(VelocityRotation::new(vec3(0.0, 1.0, 0.0), rng.gen_range(-20.0_f32..20.0).to_radians()))
                    .with_scale(Scale::new(vec3(2.0, 2.0, 2.0)))
                    .apply_choices(aabb, ecs, bounding_tree);

                ecs.write_component(*entity, angle);

                ecs.write_entity_type(*entity, TypeIdentifier::from(TypeId::of::<Asteroid>()));
            }

            entities_processed += asteroids_per_sun;
        }
    }

    if let Some(ref suns) = lock.get("blueStar")
    {
        for (_, sun_entity) in &suns.specific_instance
        {
            for entity in created_entities.iter().skip(entities_processed).take(asteroids_per_sun)
            {
                let offset = ecs.get_copy::<Position>(*sun_entity).unwrap();

                let mut angle = AngleRelativeSun
                {
                    xz_angle: rng.gen_range(0.0..360.0),
                    radius: rng.gen_range(30.0..50.0),
                    offset: vec3(offset.get_position().x, 1000.0 + rng.gen_range(-20.00..20.0), offset.get_position().z),
                    change_xz: rng.gen_range(-1.5..-0.5),
                    // radius: 20.0,
                    // offset: vec3(1000.0, 1000.0, 1020.0),
                    // xz_angle: 0.0,
                    // change_xz: 0.0
                    existence_time: 0.0
                };

                let position = calculate_position(&mut angle);

                EntityTransformationBuilder::new(*entity, false, None, false)
                    .with_translation(position)
                    .with_rotation(Rotation::new(vec3(0.0, 1.0, 0.0), 0.1_f32.to_radians()))
                    .with_rotation_velocity(VelocityRotation::new(vec3(0.0, 1.0, 0.0), rng.gen_range(-20_f32..20.01).to_radians()))
                    .with_scale(Scale::new(vec3(2.0, 2.0, 2.0)))
                    .apply_choices(aabb, ecs, bounding_tree);

                ecs.write_component(*entity, angle);

                ecs.write_entity_type(*entity, TypeIdentifier::from(TypeId::of::<Asteroid>()));
            }

            entities_processed += asteroids_per_sun;
        }
    }
}

fn calculate_position(relative_sun_angle: &mut AngleRelativeSun) -> Position
{
    let x_position = relative_sun_angle.xz_angle.to_radians().cos() * relative_sun_angle.radius + relative_sun_angle.offset.x;
    let z_position = relative_sun_angle.xz_angle.to_radians().sin() * relative_sun_angle.radius + relative_sun_angle.offset.z;

    Position::new(vec3(x_position, relative_sun_angle.offset.y, z_position))
}