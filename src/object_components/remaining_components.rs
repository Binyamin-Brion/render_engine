use nalgebra_glm::TVec3;
use super::super::ecs::{ECS, EntityId};
use crate::world::bounding_volumes::aabb::StaticAABB;
use crate::world::dimension::range::*;

type SelfEntityId = EntityId;
type CollidedIntoEntityId = EntityId;

#[derive(Copy, Clone)]
pub struct CollisionFunction
{
    pub function: fn(&mut ECS, SelfEntityId, CollidedIntoEntityId)
}

impl CollisionFunction
{
    pub fn new(function: fn(&mut ECS, SelfEntityId, CollidedIntoEntityId)) -> CollisionFunction
    {
        CollisionFunction{ function }
    }
}

#[derive(Copy, Clone)]
pub struct Currency
{
    pub currency_taken: bool,
}

impl Currency
{
    pub fn new(position: TVec3<f32>) -> (Currency, StaticAABB)
    {
        let currency_aabb_length = 10.0;

        let x_range = XRange::new(position.x - currency_aabb_length, position.x + currency_aabb_length);
        let y_range = YRange::new(position.y - currency_aabb_length, position.y + currency_aabb_length);
        let z_range = ZRange::new(position.z - currency_aabb_length, position.z + currency_aabb_length);

        (Currency{ currency_taken: false }, StaticAABB::new(x_range, y_range, z_range))
    }
}

#[derive(Copy, Clone)]
struct Position(TVec3<f32>);

#[derive(Copy, Clone)]
pub struct Radius(pub f32);

#[derive(Copy, Clone)]
pub struct WorldSection(pub u32);