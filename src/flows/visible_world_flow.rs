use std::iter::FromIterator;
use std::mem::swap;
use std::sync::Arc;
use hashbrown::HashSet;
use nalgebra_glm::TVec3;
use parking_lot::Mutex;
use rayon::iter::ParallelIterator;
use rayon::prelude::ParallelSlice;
use crate::culling::traits::TraversalDecider;
use crate::flows::shared_constants::WORLD_SECTION_LENGTH;
use crate::world::bounding_box_tree_v2::{BoundingBoxTree, UniqueWorldSectionId};
use crate::world::bounding_volumes::aabb::StaticAABB;
use crate::world::dimension::range::{XRange, YRange, ZRange};

/// Represents the logic of finding what part of the game world is visible to the camera.
pub struct VisibleWorldFlow;

impl VisibleWorldFlow
{
    pub fn find_visible_world_ids<T: TraversalDecider + Sync + Send>(frustum_culler: Arc<T>, pos: TVec3<f32>, draw: f32, bounding_tree: &BoundingBoxTree) -> Vec<UniqueWorldSectionId>
    {
        let world_aabb = StaticAABB::new
            (
                XRange::new((pos.x - draw).max(0.0), pos.x + draw),
                YRange::new((pos.y - draw).max(0.0), pos.y + draw),
                ZRange::new((pos.z - draw).max(0.0), pos.z + draw)
            );

        let mut unique_world_sections=  vec![];
        let mut level = 0;

        let world_section_length = *WORLD_SECTION_LENGTH.lock() as f32;

        while level < bounding_tree.max_level()
        {
            let level_length = world_section_length * 2.0_f32.powf(level as f32);

            let num_unique_x = (world_aabb.x_range.length() / level_length).ceil() as u32;
            let num_unique_y = (world_aabb.y_range.length() / level_length).ceil() as u32;
            let num_unique_z = (world_aabb.z_range.length() / level_length).ceil() as u32;

            let base_unique_x = (world_aabb.x_range.min / level_length) as u32;
            let base_unique_y = (world_aabb.y_range.min / level_length) as u32;
            let base_unique_z = (world_aabb.z_range.min / level_length) as u32;

            for x in 0..num_unique_x
            {
                for y in 0..num_unique_y
                {
                    for z in 0..num_unique_z
                    {
                        let id = UniqueWorldSectionId::new
                            (
                                level as u16,
                                (base_unique_x + x) as u16,
                                ( base_unique_z + z) as u16,
                                (base_unique_y + y) as u16
                            );

                        let base_x = (base_unique_x + x) as f32 * level_length;
                        let base_y = (base_unique_y + y) as f32 * level_length;
                        let base_z = (base_unique_z + z) as f32 * level_length;

                        let aabb = StaticAABB::new
                            (
                                XRange::new(base_x, base_x + level_length),
                                YRange::new(base_y, base_y + level_length),
                                ZRange::new(base_z, base_z + level_length)
                            );

                        unique_world_sections.push((id, aabb));
                    }
                }
            }

            level += 1;
        }

        let visible_ids: Arc<Mutex<Vec<UniqueWorldSectionId>>> = Arc::new(Mutex::new(Vec::new()));

        unique_world_sections.par_chunks(25).map(|x|
            {
                let mut local_visible_ids = vec![];

                for (id, aabb) in x
                {
                    if frustum_culler.aabb_in_view(aabb)
                    {
                        local_visible_ids.push(id);
                    }
                }

                visible_ids.lock().extend(local_visible_ids.into_iter());
            }).collect::<()>();

        let mut lock = visible_ids.lock();
        let mut other = vec![];
        swap(&mut *lock, &mut other);

        other
    }

    pub fn find_visible_world_ids_map<T: TraversalDecider + Sync + Send>(frustum_culler: Arc<T>, pos: TVec3<f32>, draw: f32, bounding_tree: &BoundingBoxTree) -> HashSet::<UniqueWorldSectionId>
    {
        HashSet::from_iter(VisibleWorldFlow::find_visible_world_ids(frustum_culler, pos, draw, bounding_tree).into_iter())
    }
}