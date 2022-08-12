use crate::world::bounding_volumes::aabb::StaticAABB;

/// Traits used to determine how to determine which AABBs contain models / instances that should be rendered.
pub trait TraversalDecider
{
    fn aabb_in_view(&self, aabb: &StaticAABB) -> bool;
}