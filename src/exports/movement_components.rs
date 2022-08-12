use std::ops::{AddAssign, Mul};

use nalgebra_glm::{TVec3, TMat4x4, vec3};
use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct HasMoved;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Position(TVec3<f32>);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Velocity(TVec3<f32>);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Acceleration(TVec3<f32>);

// *** Rotation ***

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct HasRotated;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Rotation(TVec3<f32>, f32);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct VelocityRotation(TVec3<f32>, f32);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct AccelerationRotation(TVec3<f32>, f32);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Scale(TVec3<f32>);

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct DynamicObject;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct TransformationMatrix(TMat4x4<f32>);

impl Default for Rotation
{
    fn default() -> Self
    {
        Rotation(vec3(1.0, 0.0, 0.0), 0.0)
    }
}

impl Default for Scale
{
    fn default() -> Self
    {
        Scale(vec3(1.0, 1.0, 1.0))
    }
}

impl Position
{
    pub fn new(position: TVec3<f32>) -> Position
    {
        debug_assert_ne!(position.x, f32::NAN, "Position (x-axis) is Nan");
        debug_assert_ne!(position.y, f32::NAN, "Position (y-axis) is Nan");
        debug_assert_ne!(position.z, f32::NAN, "Position (z-axis) is Nan");

        Position(position)
    }

    pub fn get_position(&self) -> TVec3<f32>
    {
        self.0
    }
}

impl Velocity
{
    pub fn new(velocity: TVec3<f32>) -> Velocity
    {
        debug_assert_ne!(velocity.x, f32::NAN, "Velocity (x-axis) is Nan");
        debug_assert_ne!(velocity.y, f32::NAN, "Velocity (y-axis) is Nan");
        debug_assert_ne!(velocity.z, f32::NAN, "Velocity (z-axis) is Nan");

        Velocity(velocity)
    }

    pub fn get_velocity(&self) -> TVec3<f32>
    {
        self.0
    }
}

impl Acceleration
{
    pub fn new(acceleration: TVec3<f32>) -> Acceleration
    {
        debug_assert_ne!(acceleration.x, f32::NAN, "Acceleration (x-axis) is Nan");
        debug_assert_ne!(acceleration.y, f32::NAN, "Acceleration (y-axis) is Nan");
        debug_assert_ne!(acceleration.z, f32::NAN, "Acceleration (z-axis) is Nan");

        Acceleration(acceleration)
    }

    pub fn get_acceleration(&self) -> TVec3<f32>
    {
        self.0
    }
}

impl Rotation
{
    pub fn new(rotation_axis: TVec3<f32>, rotation_radians: f32) -> Rotation
    {
        let length = nalgebra_glm::length(&rotation_axis);
        assert!(length != 0.0, "Cannot have a point rotation vector");
        assert!(length != f32::NAN, "Cannot have a NaN rotation vector");

        let normalized_axis = nalgebra_glm::normalize(&rotation_axis);
        Rotation(normalized_axis, rotation_radians)
    }

    pub fn get_rotation_axis(&self) -> TVec3<f32>
    {
        self.0
    }

    pub fn get_rotation(&self) -> f32
    {
        self.1
    }
}

impl VelocityRotation
{
    pub fn new(rotation_axis: TVec3<f32>, rotation_speed_radians: f32) -> VelocityRotation
    {
        let length = nalgebra_glm::length(&rotation_axis);
        assert!(length != 0.0, "Cannot have a point rotation vector");
        assert!(length != f32::NAN, "Cannot have a NaN rotation vector");

        let normalized_axis = nalgebra_glm::normalize(&rotation_axis);
        VelocityRotation(normalized_axis, rotation_speed_radians)
    }

    pub fn get_rotation_axis(&self) -> TVec3<f32>
    {
        self.0
    }

    pub fn get_rotation(&self) -> f32
    {
        self.1
    }
}

impl AccelerationRotation
{
    pub fn new(rotation_axis: TVec3<f32>, rotation_acceleration_radians: f32) -> AccelerationRotation
    {
        let length = nalgebra_glm::length(&rotation_axis);
        assert!(length != 0.0, "Cannot have a point rotation vector");
        assert!(length != f32::NAN, "Cannot have a NaN rotation vector");

        let normalized_axis = nalgebra_glm::normalize(&rotation_axis);
        AccelerationRotation(normalized_axis, rotation_acceleration_radians)
    }

    pub fn get_rotation_axis(&self) -> TVec3<f32>
    {
        self.0
    }

    pub fn get_rotation_acceleration(&self) -> f32
    {
        self.1
    }
}

impl Scale
{
    pub fn new(scale_amount: TVec3<f32>) -> Scale
    {
        debug_assert!(scale_amount.x >= 0.0, "Scale (x-axis) amount must equal to or greater than 0");
        debug_assert!(scale_amount.y >= 0.0, "Scale (y-axis) amount must equal to or greater than 0");
        debug_assert!(scale_amount.z >= 0.0, "Scale (z-axis) amount must equal to or greater than 0");
        Scale(scale_amount)
    }

    pub fn get_scale(&self) -> TVec3<f32>
    {
        self.0
    }
}

impl TransformationMatrix
{
    pub fn new(matrix: TMat4x4<f32>) -> TransformationMatrix
    {
        for x in 0..16
        {
            debug_assert_ne!(matrix[x], f32::NAN, "Given matrix has a NaN number");
        }
        TransformationMatrix(matrix)
    }

    pub fn get_matrix(&self) -> TMat4x4<f32>
    {
        self.0
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
                    debug_assert_ne!(self.0.x, f32::NAN, "After add assign, self (x-axis) is NaN");
                    debug_assert_ne!(self.0.y, f32::NAN, "After add assign, self (y-axis) is NaN");
                    debug_assert_ne!(self.0.z, f32::NAN, "After add assign, self (z-axis) is NaN");
                }
            }
        )+
    };
}

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
                    self.0 = nalgebra_glm::normalize(&self.0);
                    debug_assert_ne!(self.0.x, f32::NAN, "After add assign rotation, self (x-axis) is NaN");
                    debug_assert_ne!(self.0.y, f32::NAN, "After add assign rotation, self (y-axis) is NaN");
                    debug_assert_ne!(self.0.z, f32::NAN, "After add assign rotation, self (z-axis) is NaN");
                    debug_assert_ne!(self.1, f32::NAN, "After add assign, angle is NaN");
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
                    $target::new(updated_vector)
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
                    // Passing in zero results in a zero updated_vector, which when normalized
                    // in the movement component constructor causes NaN
                    assert!(rhs != 0.0, "Cannot have a zero multiplication for rotation multiplication");

                    let updated_vector = self.0 * rhs;
                    let updated_rotation = self.1 * rhs;
                    $target::new(updated_vector, updated_rotation)
                }
            }
        )+
    };
}

implement_mul_rotation!(AccelerationRotation, f32);
implement_mul_rotation!(VelocityRotation, f32);