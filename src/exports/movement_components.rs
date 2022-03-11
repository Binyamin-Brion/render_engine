use std::ops::{AddAssign, Mul};
use crate::exports::movement_forces_components::*;

use nalgebra_glm::{TVec3, TMat4x4, vec3};
use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct HasMoved;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Position(pub TVec3<f32>);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Velocity(pub TVec3<f32>);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Acceleration(pub TVec3<f32>);

// *** Rotation ***

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct HasRotated;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Rotation(pub TVec3<f32>, pub f32);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct VelocityRotation(pub TVec3<f32>, pub f32);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct AccelerationRotation(pub TVec3<f32>, pub f32);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Scale(pub TVec3<f32>);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct DynamicObject;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct TransformationMatrix(pub TMat4x4<f32>);

impl Default for Rotation
{
    fn default() -> Self
    {
        Rotation(vec3(0.0, 0.0, 0.0), 0.0)
    }
}

impl Default for Scale
{
    fn default() -> Self
    {
        Scale(vec3(1.0, 1.0, 1.0))
    }
}

macro_rules! implement_add_assign {
    ($target: ty, $($apply_to: ty),+) =>
    {
        $(
            impl AddAssign<$apply_to> for $target
            {
                fn add_assign(&mut self, rhs: $apply_to)
                {
                    self.0 += rhs.0;
                }
            }
        )+
    };
}

implement_add_assign!(Acceleration, Drag, Thrust, WormholeAppliedForce);
implement_add_assign!(Velocity, Acceleration);
implement_add_assign!(Position, Position, Velocity);

macro_rules! implement_add_assign_rotation {
    ($target: ty, $($apply_to: ty),+) =>
    {
        $(
            impl AddAssign<$apply_to> for $target
            {
                fn add_assign(&mut self, rhs: $apply_to)
                {
                    self.0 += rhs.0;
                    self.1 += rhs.1;
                }
            }
        )+
    };
}

implement_add_assign_rotation!(VelocityRotation, AccelerationRotation);
implement_add_assign_rotation!(Rotation, VelocityRotation);

macro_rules! implement_mul {
    ($target: ident, $($apply_to: ty),+) =>
    {
        $(
            impl Mul<$apply_to> for $target
            {
                type Output = $target;

                fn mul(self, rhs: f32) -> Self::Output
                {
                    let updated_vector = self.0 * rhs;
                    $target(updated_vector)
                }
            }
        )+
    };
}
implement_mul!(Acceleration, f32);
implement_mul!(Velocity, f32);

macro_rules! implement_mul_rotation {
    ($target: ident, $($apply_to: ty),+) =>
    {
        $(
            impl Mul<$apply_to> for $target
            {
                type Output = $target;

                fn mul(self, rhs: f32) -> Self::Output
                {
                    let updated_vector = self.0 * rhs;
                    let updated_rotation = self.1 * rhs;
                    $target(updated_vector, updated_rotation)
                }
            }
        )+
    };
}

implement_mul_rotation!(AccelerationRotation, f32);
implement_mul_rotation!(VelocityRotation, f32);