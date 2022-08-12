use nalgebra_glm::{TVec3, TVec4};
use serde::{Serialize, Deserialize};

pub struct DirectionLight;

pub struct PointLight;

pub struct SpotLight;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct LightInformation
{
    pub radius: f32,
    pub diffuse_colour: TVec3<f32>,
    pub specular_colour: TVec3<f32>,
    pub ambient_colour: TVec4<f32>,
    pub linear_coefficient: f32,
    pub quadratic_coefficient: f32,
    pub cutoff: Option<f32>,
    pub outer_cutoff: Option<f32>,

    pub direction: Option<TVec3<f32>>,
    pub fov: Option<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct BorderOutline;

/// The type of light to find when searching for nearby lights
#[repr(usize)]
#[derive(Copy, Clone, Deserialize, Serialize)]
pub enum FindLightType
{
    // These values correspond to the sortable component index for the given light type
    Directional = 1,
    Point = 2,
    Spot = 3
}