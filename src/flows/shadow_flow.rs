use std::collections::VecDeque;
use std::iter::FromIterator;
use std::sync::Arc;
use hashbrown::{HashMap, HashSet};
use nalgebra_glm::{TMat4, TVec3, vec3};
use crate::culling::render_frustum_culler::RenderFrustumCuller;
use crate::culling::traits::TraversalDecider;
use crate::exports::camera_object::{Camera, CameraBuilder};
use crate::exports::light_components::{FindLightType, LightInformation};
use crate::exports::movement_components::Position;
use crate::flows::visible_world_flow::{CullResult, VisibleWorldFlow};
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::world::bounding_box_tree_v2::{BoundingBoxTree, SharedWorldSectionId, UniqueWorldSectionId};
use crate::world::bounding_volumes::aabb::StaticAABB;
use crate::world::dimension::range::{XRange, YRange, ZRange};

/// Handles the logic of determine what light source needs a shadow map and the information required
/// to render the shadow map
pub struct ShadowFlow
{
    current_light_type: ServicingLightType,

    spotlights: HashMap<EntityId, ShadowMapIndex>,
    point_lights: HashMap<EntityId, ShadowMapIndex>,
    directional_lights: HashMap<EntityId, ShadowMapIndex>,

    pub upload_matrices: VecDeque<TMat4<f32>>,
    pub upload_view_matrices: VecDeque<TMat4<f32>>,
    pub upload_indexes: VecDeque<u32>,

    free_indexes: VecDeque<usize>,
}

pub type TextureArrayIndex = usize;

/// Specifics if a shadow map needs to be created, and if so what information is required to do so
pub enum ShadowMapLocation
{
    NoNewMapRequired,
    NewMapRequired(Camera, CullResult, TextureArrayIndex)
}

/// Round robin selection of what type of light source to look at when determining if a shadow map
/// needs to be created
#[derive(Debug)]
pub enum ServicingLightType
{
    DirectionalLight(Option<EntityId>),
    PointLight(Option<EntityId>),
    SpotLight(Option<EntityId>),
}

/// Indexes into the shadow map texture array for a given light source
#[derive(Copy, Clone)]
pub struct ShadowMapIndex
{
    // Array of 6 as spot lights use 6 shadow maps; point and directional only use the first index
    indexes: [Option<usize>; 6]
}

/// stores variables required to determine if a new shadow map needs to be created
pub struct CalculationArgs<'a>
{
    pub visible_sections_light: &'a CullResult,
    pub ecs: &'a ECS,
    pub tree: &'a BoundingBoxTree,
    pub camera: &'a Camera,
    pub visible_direction_lights: &'a HashSet::<EntityId>,
    pub visible_point_lights: &'a HashSet::<EntityId>,
    pub visible_spot_lights: &'a HashSet::<EntityId>,
}

/// Structure to determine what parts of the world are visible
struct Culler
{
    aabb: StaticAABB,
}

impl TraversalDecider for Culler
{
    fn aabb_in_view(&self, aabb: &StaticAABB) -> bool
    {
        self.aabb.intersect(aabb)
    }
}

impl ShadowFlow
{
    /// Creates a new shadow flow that handles the logic for the given number of possible shadow maps
    ///
    /// `number_shadow_maps` - the number of shadow maps allowed to be created
    pub fn new(number_shadow_maps: usize) -> ShadowFlow
    {
        ShadowFlow
        {
            current_light_type: ServicingLightType::DirectionalLight(None),
            spotlights: Default::default(),
            point_lights: Default::default(),
            directional_lights: Default::default(),
            free_indexes: VecDeque::from_iter((0..number_shadow_maps).into_iter()),
            upload_matrices: VecDeque::new(),
            upload_indexes: VecDeque::new(),
            upload_view_matrices: VecDeque::new()
        }
    }

    /// Finds the information required for creating a new shadow map, if required
    ///
    /// `args` - structure containing the variables required to find if a new shadow map is needed
    pub fn calculate_shadow_maps(&mut self, args: CalculationArgs) -> ShadowMapLocation
    {
        // The logic of this flow's implementation will result in a new shadow map being created
        // at most every other frame. This reduces the load on the rendering portion of the engine

        let info = match self.current_light_type
        {
            ServicingLightType::DirectionalLight(current_light) =>
                {
                    self.handle_direction_light(current_light, &args)
                }
            ServicingLightType::PointLight(current_light) =>
                {
                    self.handle_point_light(current_light, &args)
                }
            ServicingLightType::SpotLight(current_light) =>
                {
                    self.handle_spot_light(current_light, &args)
                }
        };

        info
    }

    /// Finds the required information to create a shadow map for a directional light source if needed
    ///
    /// `current_light` - the light source to find shadow map information. If none is provided, then
    ///                   one will be provided
    /// `args` - structure containing the required information to find shadow map information
    fn handle_direction_light(&mut self, mut current_light: Option<EntityId>, args: &CalculationArgs) -> ShadowMapLocation
    {
        if current_light.is_none()
        {
            if self.free_indexes.is_empty()
            {
                // Move on to next light source type; hopefully when round robin gets back to this
                // light type there will a free index for the shadow map
                self.current_light_type = ServicingLightType::PointLight(None);
                return ShadowMapLocation::NoNewMapRequired;
            }

            // The provided functions to find a nearby visible light source of the given type is not used
            // here as realistically there will not be many directional lights (and these lights should
            // be visible for most of a given scene). Faster to just query all light sources of the
            // directional type
            let directional_lights = args.ecs.get_entities_with_sortable()[1];

            for entity in directional_lights
            {
                if !self.directional_lights.contains_key(entity)
                {
                    current_light = Some(*entity);
                    self.current_light_type = ServicingLightType::DirectionalLight(current_light);
                    break;
                }
            }
        }

        match current_light
        {
            Some(entity_id) =>
                {
                    let free_index = self.free_indexes.pop_front().unwrap();

                    let position = args.ecs.get_copy::<Position>(entity_id).unwrap().get_position();
                    let light_information = args.ecs.get_copy::<LightInformation>(entity_id).unwrap();
                    let window_size = (args.camera.window_width, args.camera.window_height);

                    let light_camera = CameraBuilder::new(window_size)
                        .as_orthographic()
                        .with_position(position)
                        .with_direction(light_information.direction.unwrap())
                        .with_left_ortho(args.tree.outline_length() as f32)
                        .with_right_ortho(args.tree.outline_length() as f32)
                        .with_top_ortho(args.tree.outline_length() as f32)
                        .with_bottom_ortho(args.tree.outline_length() as f32)
                        .with_near_ortho(0.1)
                        .with_far_ortho(light_information.radius)
                        .with_far_draw_distance(light_information.radius) // Keep args into find_visible_world_ids consistent across files by using camera.get_far_draw_distance()
                        .build();

                    let render_frustum_culler = RenderFrustumCuller::new(args.camera.get_projection_matrix() * args.camera.get_view_matrix());
                    let visible_world_sections =
                        VisibleWorldFlow::find_visible_world_ids_frustum_aabb(Arc::new(render_frustum_culler), light_camera.get_position(), light_camera.get_far_draw_distance(), light_camera.get_direction(), args.tree);

                    ShadowMapLocation::NewMapRequired(light_camera, visible_world_sections, free_index)
                }
            None =>
                {
                    self.current_light_type = ServicingLightType::PointLight(None);
                    ShadowMapLocation::NoNewMapRequired
                }
        }
    }

    /// Finds the required information to create a shadow map for a point light source if needed
    ///
    /// `current_light` - the light source to find shadow map information. If none is provided, then
    ///                   one will be provided
    /// `args` - structure containing the required information to find shadow map information
    fn handle_point_light(&mut self, mut current_light: Option<EntityId>, args: &CalculationArgs) -> ShadowMapLocation
    {
        if current_light.is_none()
        {
            current_light = self.find_next_light_to_have_shadow_map(args, FindLightType::Point);
            self.current_light_type = ServicingLightType::PointLight(current_light);

            if let Some(entity_id) = current_light
            {
                self.point_lights.insert(entity_id, ShadowMapIndex{ indexes: [None; 6] });
            }
        }

        match current_light
        {
            Some(entity_id) =>
                {
                    // This is checked when finding id of light source to create map for,
                    // but check is done anyways for safety, just in case
                    let free_index = match self.free_indexes.pop_front()
                    {
                        Some(i) => i,
                        None => return ShadowMapLocation::NoNewMapRequired
                    };

                    let position = args.ecs.get_copy::<Position>(entity_id).unwrap().get_position();
                    let light_information = args.ecs.get_copy::<LightInformation>(entity_id).unwrap();
                    let window_size = (args.camera.window_width, args.camera.window_height);

                    let light_camera = CameraBuilder::new(window_size)
                        .with_near_draw_distance(0.1)
                        .with_far_draw_distance(light_information.radius)
                        .with_position(position)
                        .with_fov(light_information.fov.unwrap())
                        .with_direction(light_information.direction.unwrap())
                        .build();

                    let render_frustum_culler = RenderFrustumCuller::new(args.camera.get_projection_matrix() * args.camera.get_view_matrix());
                    let visible_world_sections =
                        VisibleWorldFlow::find_visible_world_ids_frustum_aabb(Arc::new(render_frustum_culler), light_camera.get_position(), light_camera.get_far_draw_distance(), light_camera.get_direction(), args.tree);

                    ShadowMapLocation::NewMapRequired(light_camera, visible_world_sections, free_index)
                },
            None =>
                {
                    self.current_light_type = ServicingLightType::SpotLight(None);
                    ShadowMapLocation::NoNewMapRequired
                }
        }
    }

    /// Finds the required information to create a shadow map for a spot light source if needed
    ///
    /// `current_light` - the light source to find shadow map information. If none is provided, then
    ///                   one will be provided
    /// `args` - structure containing the required information to find shadow map information
    fn handle_spot_light(&mut self, mut current_light: Option<EntityId>, args: &CalculationArgs) -> ShadowMapLocation
    {
        if current_light.is_none()
        {
            current_light = self.find_next_light_to_have_shadow_map(args, FindLightType::Spot);
            self.current_light_type = ServicingLightType::SpotLight(current_light);

            if let Some(entity_id) = current_light
            {
                self.spotlights.insert(entity_id, ShadowMapIndex{ indexes: [None; 6] });
            }
        }

        match current_light
        {
            Some(entity_id) =>
                {
                    let mut indexes = self.spotlights.get_mut(&entity_id).unwrap();

                    // Check if all six of the required shadow maps needed for spot lights have been created
                    match indexes.indexes.iter().position(|x| x.is_none())
                    {
                        Some(i) =>
                            {
                                // This is checked when finding id of light source to create map for,
                                // but check is done anyways for safety, just in case
                                let free_index = match self.free_indexes.pop_front()
                                {
                                    Some(i) => i,
                                    None => return ShadowMapLocation::NoNewMapRequired
                                };
                                indexes.indexes[i] = Some(free_index);

                                let direction_vector = match i
                                {
                                    0 => vec3(-1.0, 0.0, 0.0),
                                    1 => vec3(0.0, -1.0, 0.0),
                                    2 => vec3(0.0, 0.0, -1.0),
                                    3 => vec3(1.0, 0.0, 0.0),
                                    4 => vec3(0.0, 1.0, 0.0),
                                    5 => vec3(0.0, 0.0, 1.0),
                                    _ => unreachable!()
                                };

                                let up_vector = match i
                                {
                                    0 => vec3(0.0, -1.0, 0.0),
                                    1 => vec3(0.0, 0.0, -1.0),
                                    2 => vec3(0.0, -1.0, 0.0),
                                    3 => vec3(0.0, -1.0, 0.0),
                                    4 => vec3(0.0, 0.0, 1.0),
                                    5 => vec3(0.0, -1.0, 0.0),
                                    _ => unreachable!()
                                };

                                let position = args.ecs.get_copy::<Position>(entity_id).unwrap().get_position();
                                let far_draw_distance = args.ecs.get_copy::<LightInformation>(entity_id).unwrap().radius;
                                let light_camera = CameraBuilder::new((1024, 1024))
                                    .with_near_draw_distance(0.10)
                                    .with_far_draw_distance(far_draw_distance)
                                    .with_position(position)
                                    .with_fov(90.0)
                                    .with_direction(direction_vector)
                                    .with_up_vector(up_vector)
                                    .build();

                                let light_matrix = light_camera.get_projection_matrix() * light_camera.get_view_matrix();

                                let render_frustum_culler = RenderFrustumCuller::new(light_matrix);
                                let visible_world_sections =
                                    VisibleWorldFlow::find_visible_world_ids_frustum_aabb(Arc::new(render_frustum_culler), light_camera.get_position(), light_camera.get_far_draw_distance(), light_camera.get_direction(),args.tree);

                                self.upload_matrices.push_back(light_matrix);
                                self.upload_view_matrices.push_back(light_camera.get_view_matrix());
                                self.upload_indexes.push_back(free_index as u32);

                                ShadowMapLocation::NewMapRequired(light_camera, visible_world_sections, free_index)
                            },
                        None =>
                            {
                                self.current_light_type = ServicingLightType::DirectionalLight(None);
                                ShadowMapLocation::NoNewMapRequired
                            }
                    }
                },
            None =>
                {
                    self.current_light_type = ServicingLightType::DirectionalLight(None);
                    ShadowMapLocation::NoNewMapRequired
                }
        }
    }

    /// Determines the id of the light source that should have a shadow map created for it
    ///
    /// `args` - the structure containing variable required to create a shadow map
    /// `light_type` - the type of light for which a shadow map should be created for it
    fn find_next_light_to_have_shadow_map(&mut self, args: &CalculationArgs, light_type: FindLightType) -> Option<EntityId>
    {
        // Quick check that could save a lot of work
        if self.free_indexes.is_empty()
        {
            return None;
        }

        // These lights include both that are visible and not visible to the camera; all lights within
        // a given distance from the camera are included
        let nearby_light_sources = find_nearby_lights(&args.visible_sections_light.visible_sections_map, args.tree, light_type);

        let visible_lights = match light_type
        {
            FindLightType::Directional => args.visible_direction_lights,
            FindLightType::Point => args.visible_point_lights,
            FindLightType::Spot => args.visible_spot_lights
        };

        let target_map = match light_type
        {
            FindLightType::Directional => &mut self.directional_lights,
            FindLightType::Point => &mut self.point_lights,
            FindLightType::Spot => &mut self.spotlights
        };

        // Remove lights that are no longer visible to the camera
        let mut non_nearby_lights = Vec::new();
        for entity in target_map.keys()
        {
            // A loop is used as the .difference() method does not work between a set and a map

            if !nearby_light_sources.contains(entity)
            {
                non_nearby_lights.push(*entity);
            }
        }

        for x in non_nearby_lights
        {
            if let Some(indexes) = target_map.remove(&x)
            {
                for index in indexes.indexes.iter().filter_map(|x| *x)
                {
                    self.free_indexes.push_back(index);
                }
            }
        }

        // Find if a visible light in currently being rendered; creating shadow for it is a priority
        let priority_light =
            {
                let mut priority_light = None;
                for x in visible_lights.iter()
                {
                    if !target_map.contains_key(x)
                    {
                        priority_light = Some(*x);
                    }
                }

                priority_light
            };

        if priority_light.is_some()
        {
            return priority_light;
        }
        else
        {
            // If all visible light sources have shadow maps already, then choose a nearby offscreen
            // light source. There is a chance it will be needed soon (such as if the camera rotates)
            for x in nearby_light_sources
            {
                if !visible_lights.contains(&x)
                {
                    return Some(x);
                }
            }
        }

        // All required light sources have a shadow map
        None
    }
}

/// Finds nearby light sources (relative to the camera) that are of the given type
///
/// `camera` - the camera used for rendering
/// `bounding_box_tree` - structure that divides the world into sub-sections
/// `light_type` - the type of light that the nearby lights should be
/// `radius` - the maximum distance from the camera lights can be to be considered nearby
pub fn find_nearby_lights(visible_world_sections: &HashSet::<UniqueWorldSectionId>, bounding_box_tree: &BoundingBoxTree, light_type: FindLightType) -> HashSet::<EntityId>
{
    let mut nearby_light_sources = HashSet::default();
    let mut processed_shared_sections: HashSet::<SharedWorldSectionId>  = HashSet::default();

    let potential_world_sections = bounding_box_tree.unique_sections_with_lights.intersection(visible_world_sections);

    for world_section in potential_world_sections
    {
        if let Some(all_section_entities) = bounding_box_tree.stored_entities_indexes.get(&world_section)
        {
            nearby_light_sources.extend(all_section_entities.lights.get_light_entities(light_type));

            for shared_world_section in &all_section_entities.shared_sections_ids
            {
                if processed_shared_sections.insert(*shared_world_section)
                {
                    match bounding_box_tree.shared_section_indexes.get(shared_world_section)
                    {
                        Some(i) => nearby_light_sources.extend(i.lights.get_light_entities(light_type)),
                        None => unreachable!()
                    }
                }
            }
        }
    }

    nearby_light_sources
}

/// Calculates which world sections are visible given the camera's position and allowed distance
/// from the camera
///
/// `camera_pos` - the position of the camera
/// `radius` - maximum distance from the camera for a world section to be considered nearby
/// `bounding_box_tree` - structure that divides the world into sub-sections
pub fn find_nearby_world_sections_maps(camera_pos: TVec3<f32>, radius: f32, bounding_box_tree: &BoundingBoxTree) -> CullResult
{
    let culling_aabb = StaticAABB::new
        (
            XRange::new(camera_pos.x - radius, camera_pos.x + radius),
            YRange::new(camera_pos.y - radius, camera_pos.y + radius),
            ZRange::new(camera_pos.z - radius, camera_pos.z + radius)
        );

    let culler = Culler{ aabb: culling_aabb };

    VisibleWorldFlow::find_visible_world_ids_entire_world(Arc::new(culler), camera_pos, radius, bounding_box_tree)
}