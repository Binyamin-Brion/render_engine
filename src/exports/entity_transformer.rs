use serde::{Serialize, Deserialize};
use crate::exports::light_components::FindLightType;
use crate::exports::logic_components::CanCauseCollisions;
use crate::exports::movement_components::*;
use crate::models::model_definitions::OriginalAABB;
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;
use crate::world::bounding_volumes::aabb::StaticAABB;

/// Stores the process of creating an entity with custom physical components
#[derive(Clone, Deserialize, Serialize)]
pub struct EntityTransformationBuilder
{
    pub entity_id: EntityId,
    is_entity_static: bool,
    can_cause_collision: bool,
    light_type: Option<FindLightType>,

    translation: Option<Position>, // Original AABB must be centred around origin
velocity: Option<Velocity>,
    acceleration: Option<Acceleration>,

    rotation: Option<Rotation>,
    rotation_velocity: Option<VelocityRotation>,
    rotation_acceleration: Option<AccelerationRotation>,

    scale: Option<Scale>,
}


impl EntityTransformationBuilder
{
    pub fn new(entity_id: EntityId, is_initially_static: bool, light_type: Option<FindLightType>, can_cause_collision: bool) -> EntityTransformationBuilder
    {
        EntityTransformationBuilder
        {
            entity_id,
            is_entity_static: is_initially_static,
            can_cause_collision,
            light_type,

            translation: None,
            velocity: None,
            acceleration: None,

            rotation: None,
            rotation_velocity: None,
            rotation_acceleration: None,

            scale: None,
        }
    }

    pub fn apply_choices(&mut self, mut original_aabb: StaticAABB, ecs: &mut ECS, bounding_tree: &mut BoundingBoxTree)
    {
        self.check_invariants();

        let transformation_matrix = self.write_components(ecs);

        ecs.write_component::<OriginalAABB>(self.entity_id, OriginalAABB { aabb: original_aabb });
        let transformed_aabb = original_aabb.apply_transformation(&transformation_matrix);
        ecs.write_component::<StaticAABB>(self.entity_id, transformed_aabb);
        ecs.write_component::<TransformationMatrix>(self.entity_id, TransformationMatrix::new(transformation_matrix));

        if self.can_cause_collision
        {
            ecs.write_component::<CanCauseCollisions>(self.entity_id, CanCauseCollisions);
        }

        if bounding_tree.add_entity(self.entity_id, &transformed_aabb, false, self.is_entity_static, self.light_type).is_err()
        {
            eprintln!("Position {:?} is an invalid location", self.translation.unwrap().get_position());
        }
    }

    fn check_invariants(&mut self)
    {
        assert!(self.translation.is_some(), "A translation is required to be provided");

        if self.acceleration.is_some()
        {
            assert!(self.velocity.is_some(), "Providing acceleration requires providing velocity");
            assert!(self.translation.is_some(), "Providing acceleration requires providing a position through a translation");
        }

        if self.rotation_acceleration.is_some()
        {
            assert!(self.rotation_velocity.is_some(), "Providing rotation acceleration requires providing rotation velocity");
            assert!(self.rotation.is_some(), "Providing rotation acceleration requires providing a rotation");
        }

        if self.rotation_velocity.is_some()
        {
            assert!(self.rotation.is_some(), "Providing rotation velocity requires providing a rotation");
        }
    }

    fn write_components(&mut self, ecs: &mut ECS) -> nalgebra_glm::Mat4x4
    {
        let mut transformation_matrix = nalgebra_glm::identity();

        if let Some(translation) = self.translation
        {
            ecs.write_component::<Position>(self.entity_id,translation);
            transformation_matrix = nalgebra_glm::translate(&transformation_matrix, &translation.get_position());
        }

        if let Some(velocity) = self.velocity
        {
            ecs.write_component::<Velocity>(self.entity_id, velocity);
        }

        if let Some(acceleration) = self.acceleration
        {
            ecs.write_component::<Acceleration>(self.entity_id, acceleration);
        }

        if let Some(rotation) = self.rotation
        {
            ecs.write_component::<Rotation>(self.entity_id, rotation);
            transformation_matrix = nalgebra_glm::rotate(&transformation_matrix, rotation.get_rotation(), &rotation.get_rotation_axis());
        }

        if let Some(rotation_velocity) = self.rotation_velocity
        {
            ecs.write_component::<VelocityRotation>(self.entity_id, rotation_velocity);
        }

        if let Some(rotation_acceleration) = self.rotation_acceleration
        {
            ecs.write_component::<AccelerationRotation>(self.entity_id, rotation_acceleration);
        }

        if let Some(scale) = self.scale
        {
            ecs.write_component::<Scale>(self.entity_id, scale);
            transformation_matrix = nalgebra_glm::scale(&transformation_matrix, &scale.get_scale());
        }

        transformation_matrix
    }

    #[allow(dead_code)]
    pub fn with_translation(&mut self, translation: Position) -> &mut Self
    {
        self.translation = Some(translation);
        self
    }

    #[allow(dead_code)]
    pub fn with_velocity(&mut self, velocity: Velocity) -> &mut Self
    {
        self.velocity = Some(velocity);
        self
    }

    #[allow(dead_code)]
    pub fn with_acceleration(&mut self, acceleration: Acceleration) -> &mut Self
    {
        self.acceleration = Some(acceleration);
        self
    }

    #[allow(dead_code)]
    pub fn with_rotation(&mut self, rotation: Rotation) -> &mut Self
    {
        self.rotation = Some(rotation);
        self
    }

    #[allow(dead_code)]
    pub fn with_rotation_velocity(&mut self, velocity: VelocityRotation) -> &mut Self
    {
        self.rotation_velocity = Some(velocity);
        self
    }

    #[allow(dead_code)]
    pub fn with_rotation_acceleration(&mut self, acceleration: AccelerationRotation) -> &mut Self
    {
        self.rotation_acceleration = Some(acceleration);
        self
    }

    #[allow(dead_code)]
    pub fn with_scale(&mut self, scale: Scale) -> &mut Self
    {
        self.scale = Some(scale);
        self
    }
}