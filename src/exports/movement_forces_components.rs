use nalgebra_glm::TVec3;

#[derive(Copy, Clone, Debug)]
pub struct Drag(pub TVec3<f32>);

#[derive(Copy, Clone, Debug)]
pub struct Gravity(pub TVec3<f32>);

#[derive(Copy, Clone, Debug)]
pub struct Thrust(pub TVec3<f32>);

#[derive(Copy, Clone, Debug)]
pub struct WormholeAppliedForce(pub TVec3<f32>);