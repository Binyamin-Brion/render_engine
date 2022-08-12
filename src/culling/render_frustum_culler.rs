use nalgebra_glm::{TVec4, TMat4x4, TVec3, vec4};
use crate::culling::r#trait::TraversalDecider;
use crate::world::bounding_volumes::aabb::StaticAABB;

/// Represents the possible planes if a frustum
enum FrustumPlane
{
    Left = 0, // Indices used to index into an array of vectors representing plane normals
Right,
    Bottom,
    Top,
    Near,
    Far
}

/// Represents a frustum and the required logic to determine if a point is visible to the camera
#[derive(Clone)]
pub struct RenderFrustumCuller
{
    plane_coefficients: [TVec4<f32>; 6],
}

impl TraversalDecider for RenderFrustumCuller
{
    fn aabb_in_view(&self, aabb: &StaticAABB) -> bool
    {
        self.aabb_visible(aabb)
    }
}

impl RenderFrustumCuller
{
    /// Creates a new FrustumCuller, centred around the origin
    ///
    /// `view_projection_matrix` - the projection * view matrix of the camera
    pub fn new(mut view_projection_matrix: TMat4x4<f32>) -> RenderFrustumCuller
    {
        let plane_coefficients =
            [
                vec4(0.0, 0.0, 0.0, 0.0),
                vec4(0.0, 0.0, 0.0, 0.0),
                vec4(0.0, 0.0, 0.0, 0.0),
                vec4(0.0, 0.0, 0.0, 0.0),
                vec4(0.0, 0.0, 0.0, 0.0),
                vec4(0.0, 0.0, 0.0, 0.0),
            ];

        let mut frustum_culler = RenderFrustumCuller { plane_coefficients };
        frustum_culler.update_plane_coefficients(&mut view_projection_matrix);
        frustum_culler
    }

    /// Extracts the frustum plane coefficient from the given view projection matrix, and centres
    /// the frustum plane at the given location.
    ///
    /// Should be called whenever the camera moves or rotates
    ///
    /// `view_projection_matrix` - the projection * view matrix of the camera
    fn update_plane_coefficients(&mut self, view_projection_matrix: &mut TMat4x4<f32>)
    {
        *view_projection_matrix = nalgebra_glm::transpose(view_projection_matrix);

        let column = |x: usize|{ view_projection_matrix.column(x) };

        let normalize_coefficients = |x: TVec4<f32>|
            {
                let length = nalgebra_glm::length(&nalgebra_glm::vec4_to_vec3(&x));

                x / length
            };

        self.plane_coefficients[FrustumPlane::Left as usize] = normalize_coefficients(column(3) + column(0));
        self.plane_coefficients[FrustumPlane::Right as usize] = normalize_coefficients(column(3) - column(0));
        self.plane_coefficients[FrustumPlane::Bottom as usize] = normalize_coefficients(column(3) + column(1));
        self.plane_coefficients[FrustumPlane::Top as usize] = normalize_coefficients(column(3) - column(1));
        self.plane_coefficients[FrustumPlane::Near as usize] = normalize_coefficients(column(3) - vec4(0.0, 0.0, 0.0, 0.0));
        self.plane_coefficients[FrustumPlane::Far as usize] = normalize_coefficients(column(3) - column(2));
    }

    /// Checks if the given AABB is visible in the given frustum
    ///
    /// `aabb` - the bounding volume to check for visiblity
    pub fn aabb_visible(&self, aabb: &StaticAABB) -> bool
    {
        let point_in_frustum = |plane_normal: &TVec4<f32>, point: &TVec3<f32>|
            {
                let distance_to_plane = plane_normal.x * point.x +
                    plane_normal.y * point.y +
                    plane_normal.z * point.z +
                    plane_normal.w;

                if distance_to_plane < 0.0
                {
                    return false;
                }

                true
            };

        let aabb_points = aabb.get_aabb_points();

        for x in &self.plane_coefficients
        {
            let mut all_points_inside_frustum = false;

            for point in &aabb_points
            {
                all_points_inside_frustum |= point_in_frustum(x, point);
            }

            if !all_points_inside_frustum
            {
                return false;
            }
        }

        true
    }
}