use nalgebra_glm::{TVec3, vec3, vec4};
use serde::{Serialize, Deserialize};
use crate::world::dimension::range::{XRange, YRange, ZRange};

/// Represents a bounding volume in a 3D space
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct StaticAABB
{
    pub x_range: XRange,
    pub y_range: YRange,
    pub z_range: ZRange
}

impl StaticAABB
{
    /// Creates a new AABB that occupies the given ranges
    ///
    /// `x_range` - the space the bounding volume takes in the x dimension
    /// `y_range` - the space the bounding volume takes in the y dimension
    /// `z_range` - the space the bounding volume takes in the z dimension
    #[allow(dead_code)]
    pub fn new(x_range: XRange, y_range: YRange, z_range: ZRange) -> StaticAABB
    {
        StaticAABB{ x_range, y_range, z_range }
    }

    /// Move the bounding volume in the given direction
    ///
    /// `move_vector` - vector specifying how much to move the volume in each dimension
    #[allow(dead_code)]
    pub fn translate(&mut self, move_vector: TVec3<f32>)
    {
        self.x_range.translate(move_vector.x);
        self.y_range.translate(move_vector.y);
        self.z_range.translate(move_vector.z);
    }

    /// Merges two AABBs to form one that could hold of the given AABB
    ///
    /// `other_aabb` - the AABB to merge with this one
    #[allow(dead_code)]
    pub fn combine_aabb(&self, other_aabb: &StaticAABB) -> StaticAABB
    {
        StaticAABB::new
            (
                self.x_range.combine(&other_aabb.x_range),
                self.y_range.combine(&other_aabb.y_range),
                self.z_range.combine(&other_aabb.z_range)
            )
    }

    /// Get the centre of the bounding volume
    #[allow(dead_code)]
    pub fn centre(&self) -> TVec3<f32>
    {
        vec3
            (
                self.x_range.centre(),
                self.y_range.centre(),
                self.z_range.centre()
            )
    }

    /// Check if the other AABB overlaps with this bounding volume
    ///
    /// `other_aabb` - the volume to check for an overlap with this one
    #[allow(dead_code)]
    pub fn intersect(&self, other_aabb: &StaticAABB) -> bool
    {
        self.x_range.overlap_range(&other_aabb.x_range) &&
            self.y_range.overlap_range(&other_aabb.y_range) &&
            self.z_range.overlap_range(&other_aabb.z_range)
    }

    /// Scales the bounding volume by the given amount
    ///
    /// `factor` - vector specifying how much to scale the volume in each dimension
    #[allow(dead_code)]
    pub fn scale(&mut self, factor: TVec3<f32>)
    {
        self.x_range.min *= factor.x;
        self.x_range.max *= factor.x;

        self.y_range.min *= factor.y;
        self.y_range.max *= factor.y;

        self.z_range.min *= factor.z;
        self.z_range.max *= factor.z;
    }

    /// Transform the volume by the transformation matrix. Since the vertices may no longer be axis aligned,
    /// a new AABB that is, and can hold the transformed AABB, is returned
    ///
    /// `transformation` - the transformation to apply to this AABB
    pub fn apply_transformation(&mut self, transformation: &nalgebra_glm::Mat4x4) -> StaticAABB
    {
        let first_corner = transformation * vec4(self.x_range.min, self.y_range.min, self.z_range.min, 1.0);
        let second_corner = transformation * vec4(self.x_range.max, self.y_range.max, self.z_range.max, 1.0);

        let min_x = first_corner.x.min(second_corner.x);
        let min_y = first_corner.y.min(second_corner.y);
        let min_z = first_corner.z.min(second_corner.z);

        let max_x = first_corner.x.max(second_corner.x);
        let max_y = first_corner.y.max(second_corner.y);
        let max_z = first_corner.z.max(second_corner.z);

        StaticAABB::new
            (
                XRange::new(min_x, max_x),
                YRange::new(min_y, max_y),
                ZRange::new(min_z, max_z)
            )
    }

    /// Get a default AABB centred at the origin, and has no length
    pub fn point_aabb() -> StaticAABB
    {
        StaticAABB::new
            (
                XRange::new(0.0, 0.0),
                YRange::new(0.0, 0.0),
                ZRange::new(0.0, 0.0),
            )
    }

    /// Get the points that make up this AABB
    pub fn get_aabb_points(&self) -> [TVec3<f32>; 8]
    {
        [
            vec3(self.x_range.min, self.y_range.min, self.z_range.min),
            vec3(self.x_range.min, self.y_range.min, self.z_range.max),
            vec3(self.x_range.min, self.y_range.max, self.z_range.min),
            vec3(self.x_range.min, self.y_range.max, self.z_range.max),
            vec3(self.x_range.max, self.y_range.min, self.z_range.min),
            vec3(self.x_range.max, self.y_range.min, self.z_range.max),
            vec3(self.x_range.max, self.y_range.max, self.z_range.min),
            vec3(self.x_range.max, self.y_range.max, self.z_range.max)
        ]
    }
}