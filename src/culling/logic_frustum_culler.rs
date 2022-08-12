use nalgebra_glm::TVec3;
use crate::culling::r#trait::TraversalDecider;
use crate::world::bounding_volumes::aabb::StaticAABB;

/// Determines if a bounding volume is near enough to the camera to have any entities within have
/// their logic executed
#[derive(Clone)]
pub struct LogicFrustumCuller
{
    lookahead_distance: f32,
    camera_position: TVec3<f32>,
}

impl LogicFrustumCuller
{
    /// Creates a new LogicFrustumCuller with the provided data. No default values are given for any
    /// members of the structure
    ///
    /// `lookahead_distance` - the max distance in any direction from the camera that a point is
    ///                         considered visible
    /// `camera_position` - the position of the camera
    pub fn new(lookahead_distance: f32, camera_position: TVec3<f32>) -> LogicFrustumCuller
    {
        LogicFrustumCuller{ lookahead_distance, camera_position }
    }
}

impl TraversalDecider for LogicFrustumCuller
{
    /// Determines if a bounding volume is close enough to the camera to have the logic of the entities
    /// within that volume to be executed
    fn aabb_in_view(&self, aabb: &StaticAABB) -> bool
    {
        let mut distance_to_closest_point = f32::MAX;
        let aabb_points = aabb.get_aabb_points();

        for x in &aabb_points
        {
            distance_to_closest_point = distance_to_closest_point.min(nalgebra_glm::distance(x, &self.camera_position));
        }

        // The bounding volumes very close to the camera should always be considered "visible", as otherwise
        // if the camera turns unexpected state of entities will be seen (if they are close to the camera,
        // player will probably notice if entities did not execute their logic)
        distance_to_closest_point <= self.lookahead_distance
    }
}