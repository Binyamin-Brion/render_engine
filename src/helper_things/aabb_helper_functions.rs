use nalgebra_glm::{TVec3};
use crate::world::bounding_volumes::aabb::StaticAABB;
use crate::world::dimension::range::{XRange, YRange, ZRange};

/// Calculates the enclosing AABB for the given set of vertices. This function will only provide
/// one AABB for the entire set of vertices, so it may not be tight
///
/// `vertices` - the points for which to calculate a bounding volume
pub fn calculate_aabb(vertices: &[TVec3<f32>]) -> StaticAABB
{
    let mut min_x: f32 = f32::MAX;
    let mut min_y: f32 = f32::MAX;
    let mut min_z: f32 = f32::MAX;

    let mut max_x: f32 = f32::MIN;
    let mut max_y: f32 = f32::MIN;
    let mut max_z: f32 = f32::MIN;

    for vertex in vertices
    {
        min_x = min_x.min(vertex.x);
        min_y = min_y.min(vertex.y);
        min_z = min_z.min(vertex.z);

        max_x = max_x.max(vertex.x);
        max_y = max_y.max(vertex.y);
        max_z = max_z.max(vertex.z);
    }

    StaticAABB::new
        (
            XRange::new(min_x, max_x),
            YRange::new(min_y, max_y),
            ZRange::new(min_z, max_z)
        )
}

/// Determines if a bounding volume is outside of the valid game world
///
/// `aabb` - the bounding volume for which to determine if it's out of bounds
/// `game_world_length` - how long the game world extends from the origin (assumes all dimensions
///                      extend from the origin equally)
pub fn aabb_out_of_bounds(aabb: &StaticAABB, game_world_length: f32) -> bool
{
    aabb.x_range.min < 0.0 ||
        aabb.y_range.min < 0.0 ||
        aabb.z_range.min < 0.0 ||

        aabb.x_range.max > game_world_length ||
        aabb.y_range.max > game_world_length ||
        aabb.z_range.max > game_world_length
}

/// Determines closest distance between the given point and any point on the bounding volume
///
/// `aabb` - the bounding volume to find the distance to
/// `target_pos` - the point to use to find the distance to any point on the bounding volume
pub fn distance_to_aabb(aabb: &StaticAABB, target_pos: TVec3<f32>) -> f32
{
    let compute_aabb_radius = |aabb: &StaticAABB|
        {
            let largest_length = aabb.x_range.length().max(aabb.y_range.length()).max(aabb.z_range.length());
            ((largest_length / 2.0).powi(2) * 3.0).sqrt()
        };

    let bounding_sphere_length = compute_aabb_radius(aabb);
    let distance_to_aabb_centre: f32 = nalgebra_glm::length(&(&target_pos - aabb.centre()));

    // Technically not quite the closest if AABB point that's closest is not one of the AABB corners,
    // but it's good enough and cheap to compute
    (distance_to_aabb_centre - bounding_sphere_length).max(0.0)
}