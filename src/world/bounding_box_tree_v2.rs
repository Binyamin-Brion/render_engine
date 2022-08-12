use hashbrown::{HashMap, HashSet};
use serde::{Serialize, Deserialize};
use crate::culling::r#trait::TraversalDecider;
use crate::exports::light_components::FindLightType;
use crate::helper_things::aabb_helper_functions;
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::world::bounding_volumes::aabb::StaticAABB;
use crate::world::dimension::range::{XRange, YRange, ZRange};

/// Represents a unique world section at a given level
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
struct SectionOffsets
{
    x: u16,
    z: u16,
    y: u16
}

/// Unique identifier for a subsection of the game world
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UniqueWorldSectionId
{
    level: u16, // Level increases as atomic section length increases
index: SectionOffsets,
}

impl UniqueWorldSectionId
{
    /// Creates a new world section index from the given information
    ///
    /// `level` - the level of the world section. A higher level world section represent a level with
    ///           bigger world sections
    /// `x` - the x-offset at the given level
    /// `z`- the z-offset at the given level
    /// `y` - the y-offset at the given level
    pub fn new(level: u16, x: u16, z: u16, y: u16) -> UniqueWorldSectionId
    {
        UniqueWorldSectionId { level, index: SectionOffsets {x, z, y} }
    }

    /// Computes the next higher world section index that contain this world section index. If
    /// the index is already at the max level, then this option returns None
    ///
    /// `max_level` - the maximum possible level in the tree
    pub fn higher_level_world_section(&self, max_level: u16) -> Option<UniqueWorldSectionId>
    {
        if self.level == max_level
        {
            return None;
        }

        Some(
            UniqueWorldSectionId
            {
                level: self.level + 1,
                index: SectionOffsets
                {
                    x: self.index.x / 2,
                    z: self.index.z / 2,
                    y: self.index.y / 2
                }
            })
    }

    /// Compute the next lower level child world sections of this world section
    pub fn lower_level_world_section(&self) -> Option<[UniqueWorldSectionId; 8]>
    {
        if self.level == 0
        {
            return None;
        }

        let base_x = self.index.x * 2;
        let base_z = self.index.z * 2;
        let base_y = self.index.y * 2;

        Some(
            [
                UniqueWorldSectionId::new(self.level - 1, base_x, base_z, base_y),
                UniqueWorldSectionId::new(self.level - 1, base_x + 1, base_z, base_y),
                UniqueWorldSectionId::new(self.level - 1, base_x, base_z + 1, base_y),
                UniqueWorldSectionId::new(self.level - 1, base_x + 1, base_z + 1, base_y),

                UniqueWorldSectionId::new(self.level - 1, base_x, base_z, base_y + 1),
                UniqueWorldSectionId::new(self.level - 1, base_x + 1, base_z, base_y + 1),
                UniqueWorldSectionId::new(self.level - 1, base_x, base_z + 1, base_y + 1),
                UniqueWorldSectionId::new(self.level - 1, base_x + 1, base_z + 1, base_y + 1),
            ])
    }

    /// Gets the corresponding bounding volume that corresponds to this world section index
    ///
    /// `atomic_length` - the smallest possible length of a section in the tree
    fn to_aabb(&self, atomic_length: u32) -> StaticAABB
    {
        let side_length = (2_u32.pow(self.level as u32) * atomic_length) as f32;

        let min_x = side_length * self.index.x as f32;
        let min_y = side_length * self.index.y as f32;
        let min_z = side_length * self.index.z as f32;

        StaticAABB::new
            (
                XRange::new(min_x, min_x + side_length),
                YRange::new(min_y, min_y + side_length),
                ZRange::new(min_z, min_z + side_length)
            )
    }
}

/// Represents an identifier for a section that multiple unique world sections can refer to
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SharedWorldSectionId
{
    level: u16,
    // Array of options are used as either 2 or 4 different world sections can make up a shared section,
    // and using a vector (heap) or such small allocations is likely not ideal
    indexes: [Option<SectionOffsets>; NUMBER_CONTRIBUTING_UNIQUE_SECTIONS],
}

const NUMBER_CONTRIBUTING_UNIQUE_SECTIONS: usize = 8;

impl SharedWorldSectionId
{
    /// Creates a new world section index from the given information
    ///
    /// `base_world_sections` - the world sections that compromise this shared world section
    pub fn new(base_world_sections: &Vec<UniqueWorldSectionId>) -> SharedWorldSectionId
    {
        let level = base_world_sections[0].level;
        let mut indexes = [None; NUMBER_CONTRIBUTING_UNIQUE_SECTIONS];

        for (index, world_section) in base_world_sections.iter().enumerate()
        {
            assert_eq!(level, world_section.level, "Incorrect sub-world section level- expected {}, got {}", level, world_section.level);
            indexes[index] = Some(world_section.index);
        }

        SharedWorldSectionId { level, indexes }
    }

    /// Converts the shared world section to the unique world sections that compromise it
    pub fn to_world_sections(&self) -> [Option<UniqueWorldSectionId>; NUMBER_CONTRIBUTING_UNIQUE_SECTIONS]
    {
        let mut result = [None; NUMBER_CONTRIBUTING_UNIQUE_SECTIONS];

        for (index, i) in self.indexes.iter().filter_map(|x| *x).enumerate()
        {
            result[index] = Some(UniqueWorldSectionId::new(self.level, i.x, i.z, i.y))
        }

        result
    }
}

/// Stores the light entities contained within a world section, unique or shared
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightEntities
{
    directional: HashSet::<EntityId>,
    spot: HashSet::<EntityId>,
    point: HashSet::<EntityId>
}

impl LightEntities
{
    /// Creates a new instance of LightEntities with no light entities
    fn new() -> LightEntities
    {
        LightEntities
        {
            directional: HashSet::default(),
            spot: HashSet::default(),
            point: HashSet::default()
        }
    }

    /// Add a light_entity to this world section
    ///
    /// `entity_id` - the id of the light source
    /// `light_type` - what type of light this entity is
    fn add_light_entity(&mut self, entity_id: EntityId, light_type: Option<FindLightType>)
    {
        if let Some(light_type) = light_type
        {
            match light_type
            {
                FindLightType::Directional => self.directional.insert(entity_id),
                FindLightType::Point => self.point.insert(entity_id),
                FindLightType::Spot => self.spot.insert(entity_id),
            };
        }
    }

    /// Remove the given entity from the light sources in ties world section
    ///
    /// `entity_id` - the id of tne light source to remove
    fn remove_light_entity(&mut self, entity_id: EntityId)
    {
        if !self.spot.remove(&entity_id)
        {
            if !self.point.remove(&entity_id)
            {
                self.directional.remove(&entity_id);
            }
        }
    }

    /// Get the light entities of the specified type for this world section
    ///
    /// `light_type` - the type of lights to retrieve
    pub fn get_light_entities(&self, light_type: FindLightType) -> &HashSet::<EntityId>
    {
        match light_type
        {
            FindLightType::Directional => &self.directional,
            FindLightType::Point => &self.point,
            FindLightType::Spot => &self.spot
        }
    }

    /// Determines if there are any light sources in this world section
    fn is_empty(&self) -> bool
    {
        self.point.is_empty() && self.spot.is_empty() && self.directional.is_empty()
    }
}

/// Representation of the entities in a unique world section
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UniqueWorldSectionEntities
{
    pub aabb: StaticAABB,
    back_up_aabb: StaticAABB, // Used when a lot of entities are in this world section; see end_of_changes()
pub local_entities: HashSet::<EntityId>,
    pub static_entities: HashSet::<EntityId>,
    pub shared_sections_ids: HashSet::<SharedWorldSectionId>,
    pub lights: LightEntities,
}

impl UniqueWorldSectionEntities
{
    fn is_key_to_shared_section_light(&self, shared_sections: &HashSet::<SharedWorldSectionId>) -> bool
    {
        let mut not_key_to_shared_section_light = true;

        for x in &self.shared_sections_ids
        {
            if shared_sections.contains(x)
            {
                not_key_to_shared_section_light = false;
                break;
            }
        }

        !not_key_to_shared_section_light
    }
}

/// Stores the entities in a section referred to by multiple world sections, and keeps track of
/// which unique world sections point to the shared section
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedWorldSectionEntities
{
    pub entities: HashSet::<EntityId>,
    pub static_entities: HashSet::<EntityId>,
    entity_aabb_lookup: HashMap<EntityId, StaticAABB>,
    pub aabb: StaticAABB,
    pub lights: LightEntities,
}

impl SharedWorldSectionEntities
{
    /// Creates a new SharedSectionEntities with no entities, and a point AABB
    fn new() -> SharedWorldSectionEntities
    {
        SharedWorldSectionEntities
        {
            entities: HashSet::default(),
            static_entities: HashSet::default(),
            entity_aabb_lookup: HashMap::default(),
            aabb: StaticAABB::point_aabb(),
            lights: LightEntities::new()
        }
    }

    /// Add an entity to this shared section
    ///
    /// `entity_id` - the id of the entity being added
    /// `aabb` - the bounding volume of the added entity
    fn add_entity(&mut self, entity_id: EntityId, aabb: StaticAABB, is_static: bool)
    {
        if is_static
        {
            self.static_entities.insert(entity_id);
        }
        else
        {
            self.entities.insert(entity_id);
        }

        self.entity_aabb_lookup.insert(entity_id, aabb);
    }

    /// Remove an entity from this shared section
    ///
    /// `entity_id` - the entity to remove
    fn remove_entity(&mut self, entity_id: EntityId)
    {
        if !self.entities.remove(&entity_id)
        {
            self.static_entities.remove(&entity_id);
        }

        self.entity_aabb_lookup.remove(&entity_id);
    }
}

/// Used to find which world section, unique or shared, an entity is located in
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorldSectionLookup
{
    Shared(SharedWorldSectionId),
    Unique(UniqueWorldSectionId),
}

/// Keeps track of where entities are located in the game world
#[derive(Clone, Serialize, Deserialize)]
pub struct BoundingBoxTree
{
    pub entities_index_lookup: HashMap<EntityId, WorldSectionLookup>,
    pub stored_entities_indexes: HashMap<UniqueWorldSectionId, UniqueWorldSectionEntities>,
    pub related_world_sections: HashMap<UniqueWorldSectionId, Vec<UniqueWorldSectionId>>,
    pub shared_section_indexes: HashMap<SharedWorldSectionId, SharedWorldSectionEntities>,
    pub reverse_shared_section_lookup: HashMap<SharedWorldSectionId, Vec<UniqueWorldSectionId>>,
    pub unique_sections_with_lights: HashSet::<UniqueWorldSectionId>,
    shared_section_lights: HashSet::<SharedWorldSectionId>,
    static_world_sections: HashSet::<UniqueWorldSectionId>,
    changed_static_unique_sections: HashSet::<UniqueWorldSectionId>,
    outline_length: u32,
    atomic_section_length: u32,

    changed_shared_sections: HashSet::<SharedWorldSectionId>,
    changed_world_sections: HashSet::<UniqueWorldSectionId>,
    total_world_aabb_combining: u32,
}

/// Stores the location of nearby entities when searching for related entities to a given entity
#[derive(Debug, Eq, PartialEq)]
pub struct RelatedEntitySearchResult<'a>
{
    pub location: WorldSectionLookup,
    pub entities: &'a HashSet::<EntityId>,
    pub static_entities: &'a HashSet::<EntityId>
}

impl BoundingBoxTree
{
    /// Creates a new bounding tree representing the game world with the supplied parameters
    ///
    /// `outline_length` - the max boundary of the game world, in the range of [0, outline_length]
    /// `atomic_section_length` - the smallest possible length of a section that the game world can be divided into
    pub fn new(outline_length: u32, atomic_section_length: u32) -> BoundingBoxTree
    {
        BoundingBoxTree
        {
            entities_index_lookup: HashMap::default(),
            stored_entities_indexes: HashMap::default(),
            related_world_sections: HashMap::default(),
            shared_section_indexes: HashMap::default(),
            reverse_shared_section_lookup: HashMap::default(),
            static_world_sections: HashSet::default(),
            changed_static_unique_sections: HashSet::default(),
            unique_sections_with_lights: HashSet::default(),
            shared_section_lights: HashSet::default(),
            outline_length,
            atomic_section_length,
            changed_shared_sections: HashSet::default(),
            changed_world_sections: HashSet::default(),
            total_world_aabb_combining: 0
        }
    }

    /// Checks if the given world sections exists, meaning that either it has an entity in it or
    /// is a key to a shared world section
    ///
    /// `section` - the world section to check for existence
    pub fn is_section_in_existence(&self, section: &UniqueWorldSectionId) -> bool
    {
        self.stored_entities_indexes.contains_key(section)
    }

    /// Finds if the given section has entities that need to have their logic updates
    ///
    /// `section` - the unique world section that will be checked for active entities
    pub fn is_section_active(&self, section: UniqueWorldSectionId) -> bool
    {
        self.stored_entities_indexes.contains_key(&section) &&
            !self.static_world_sections.contains(&section)
    }

    /// Get all of the ids of the static world sections that have changed (change in number of static entities)
    pub fn get_changed_static_unique(&self) -> &HashSet::<UniqueWorldSectionId>
    {
        &self.changed_static_unique_sections
    }

    /// Clear the list of changed static world section ids
    pub fn clear_changed_static_unique(&mut self)
    {
        self.changed_static_unique_sections.clear();
    }

    /// Determines if an entity is static or is active
    ///
    /// `entity_id` - the entity to query

    pub fn is_entity_static(&self, entity_id: EntityId) -> Option<bool>
    {
        if let Some(section_lookup) = self.entities_index_lookup.get(&entity_id)
        {
            return match section_lookup
            {
                WorldSectionLookup::Unique(i) => Some(self.stored_entities_indexes.get(i)?.static_entities.contains(&entity_id)),
                WorldSectionLookup::Shared(i) => Some(self.shared_section_indexes.get(i)?.static_entities.contains(&entity_id)),
            }
        }

        None
    }

    /// Get the length of the game world that this bounding tree is representing
    pub fn outline_length(&self) -> u32
    {
        self.outline_length
    }

    /// Get the smallest length if a section that the game world can be divided into

    pub fn atomic_world_section_length(&self) -> u32
    {
        self.atomic_section_length
    }

    /// Find the world section that completely encloses the given bounding volume. This function assumes
    /// the the volume is located in such a position that it is only within one world section
    ///
    /// `bounding_volume` - the bounding volume to find the world section id for
    /// `atomic_section_length` - the smallest possible length of a world section
    fn find_unique_world_section_id(bounding_volume: StaticAABB, atomic_section_length: u32) -> UniqueWorldSectionId
    {
        let (level, adjusted_atomic_length) = BoundingBoxTree::find_aabb_level_from_length_and_origin(&bounding_volume, atomic_section_length);

        let x = bounding_volume.x_range.min as u32 / adjusted_atomic_length;
        let z = bounding_volume.z_range.min  as u32/ adjusted_atomic_length;
        let y = bounding_volume.y_range.min as u32 / adjusted_atomic_length;

        UniqueWorldSectionId::new(level as u16, x as u16, z as u16, y as u16)
    }

    /// Get the all of the unique world sections that partially cover the provided bounding volume
    ///
    /// `bounding_volume` - the bounding volume for which to find the unique world sections ids that
    ///                     correspond to it
    pub fn find_all_unique_world_section_ids(&self, bounding_volumes: &StaticAABB) -> Vec<UniqueWorldSectionId>
    {
        // THe shared section is shared between unique world sections that could completely contain the bounding
        // volume if the position of the volume was different
        let (level, adjusted_atomic_length) = BoundingBoxTree::find_aabb_level_from_length(bounding_volumes, self.atomic_section_length);

        // Depending on the size of the entity's AABB, it may span more than world section of the
        // bounding tree. Thus the number of world sections required to cover the AABB in each dimension
        // is calculated.
        let (num_x, num_y, num_z) = BoundingBoxTree::calculate_number_world_sections_each_dimension(adjusted_atomic_length, &bounding_volumes);
        let mut world_sections_ids = Vec::new();

        // Use as many world section as required in order to hold the entity AABB
        for x in 0..num_x
        {
            for y in 0..num_y
            {
                for z in 0..num_z
                {
                    // Find the indexes in each dimension of all points of the entity AABB closest to the origin
                    // that are in a unique world section
                    let (index_x, index_y, index_z) = BoundingBoxTree::calculate_aabb_section_indexes(x, y, z, &bounding_volumes, adjusted_atomic_length);

                    let world_section_index = UniqueWorldSectionId::new
                        (
                            level as u16,
                            index_x as u16,
                            index_z as u16,
                            index_y as u16
                        );

                    world_sections_ids.push(world_section_index);
                }
            }
        }

        assert!(world_sections_ids.len() <= NUMBER_CONTRIBUTING_UNIQUE_SECTIONS,
                "Shared entity cannot take more than four world sections at the correct level- the AABB {:?} took {}", bounding_volumes, world_sections_ids.len());

        world_sections_ids
    }

    /// Finds the AABB level and the appropriate section length assuming the AABB is centred at the origin
    ///
    /// `bounding_volume` - find the world level that completely can completely encompass the bounding
    ///                     volume as well as the world section length that corresponds to the returned level
    /// `atomic_section_length` - the length of the smallest world section
    fn find_aabb_level_from_length(bounding_volume: &StaticAABB, atomic_section_length: u32) -> (u32, u32)
    {
        let length_x = bounding_volume.x_range.length();
        let length_y = bounding_volume.y_range.length();
        let length_z = bounding_volume.z_range.length();

        // Creating new AABB ensures that the AABB is truly at the origin, and not slightly different
        // due to floating point precision. It may not matter here, but it is safer to do so anyways
        let bounding_volume = StaticAABB::new
            (
                XRange::new(0.0, length_x),
                YRange::new(0.0, length_y),
                ZRange::new(0.0, length_z),
            );

        BoundingBoxTree::find_aabb_level_from_length_and_origin(&bounding_volume, atomic_section_length)
    }

    /// Calculate the level of world section that encompasses the given bounding volume and the length
    /// of world section at that level
    ///
    /// `bounding_volume` - find the world level that completely can completely encompass the bounding
    ///                     volume as well as the world section length that corresponds to the returned level
    /// `atomic_section_length` - the length of the smallest world section
    fn find_aabb_level_from_length_and_origin(bounding_volume: &StaticAABB, atomic_section_length: u32) -> (u32, u32)
    {
        let mut adjusted_atomic_length = atomic_section_length;
        let mut level = 0;

        let mut number_worlds_sections = BoundingBoxTree::calculate_number_world_sections_total(adjusted_atomic_length, &bounding_volume);
        while number_worlds_sections > 1
        {
            adjusted_atomic_length *= 2;
            level += 1;
            number_worlds_sections = BoundingBoxTree::calculate_number_world_sections_total(adjusted_atomic_length, &bounding_volume);
        }

        (level, adjusted_atomic_length)
    }

    /// Adds the given entity to the bounding volume. If the given AABB is out of the space defined to
    /// be the game world, then the AABB is resized to not exceed this space. Exceeding this space is
    /// referred to as out of bounds.
    ///
    /// `entity_id` - the id of the entity to add to the tree
    /// `bounding_volume` - the volume represents the physical space used by the entity
    /// `add_if_out_bounds` - if true, an out of bounds AABB is considered an error and the entity is
    ///                       not added to the bounding tree
    /// `is_static` - true if the added entity is a static object
    /// `light_type` - the type of light the entity is representing, if any
    pub fn add_entity(&mut self, entity_id: EntityId, bounding_volume: &StaticAABB, add_if_out_bounds: bool, is_static: bool, light_type: Option<FindLightType>) -> Result<(), ()>
    {
        let mut bounding_volume = bounding_volume.clone();

        let out_of_bounds = BoundingBoxTree::normalize_aabb(&mut bounding_volume, self.outline_length as f32);

        if out_of_bounds && !add_if_out_bounds
        {
            return Err(());
        }

        // Need to check first how many world sections the AABB takes to know if it should go in a shared section
        // or a unique world section
        let shared_sections = self.find_all_unique_world_section_ids(&bounding_volume);

        // The bounding volume takes more than one world section at the appropriate world division level
        if shared_sections.len() != 1
        {
            let shared_section_index = SharedWorldSectionId::new(&shared_sections);

            if self.entity_exists_in_section(entity_id, &WorldSectionLookup::Shared(shared_section_index))
            {
                return Ok(());
            }

            if is_static
            {
                for unique_world_section in &shared_sections
                {
                    self.changed_static_unique_sections.insert(*unique_world_section);
                }
            }

            match self.shared_section_indexes.get_mut(&shared_section_index)
            {
                Some(i) =>
                    {
                        i.add_entity(entity_id, bounding_volume, is_static);
                        i.lights.add_light_entity(entity_id, light_type);

                        if light_type.is_some()
                        {
                            self.shared_section_lights.insert(shared_section_index);

                            for x in shared_section_index.to_world_sections().iter().filter_map(|x| *x)
                            {
                                self.unique_sections_with_lights.insert(x);
                            }
                        }

                        self.entities_index_lookup.insert(entity_id, WorldSectionLookup::Shared(shared_section_index));
                    },
                None =>
                    {
                        // Add entry for shared section containing the passed in entity id
                        let added_section = self.shared_section_indexes.entry(shared_section_index).or_insert(SharedWorldSectionEntities::new());
                        added_section.add_entity(entity_id, bounding_volume, is_static);
                        added_section.lights.add_light_entity(entity_id, light_type);
                        if light_type.is_some()
                        {
                            self.shared_section_lights.insert(shared_section_index);

                            for x in shared_section_index.to_world_sections().iter().filter_map(|x| *x)
                            {
                                self.unique_sections_with_lights.insert(x);
                            }
                        }

                        // Specify where to find the entity given its id
                        self.entities_index_lookup.insert(entity_id, WorldSectionLookup::Shared(shared_section_index));

                        for world_section_id in shared_sections
                        {
                            // Register each world section that shares the shared world section with the shared section
                            match self.stored_entities_indexes.get_mut(&world_section_id)
                            {
                                Some(i) =>
                                    {
                                        i.shared_sections_ids.insert(shared_section_index);
                                    },
                                None =>
                                    {
                                        let mut stored_entities_indexes = UniqueWorldSectionEntities
                                        {
                                            aabb: StaticAABB::point_aabb(),
                                            back_up_aabb: world_section_id.to_aabb(self.atomic_section_length),
                                            local_entities: HashSet::default(),
                                            static_entities: HashSet::default(),
                                            shared_sections_ids: HashSet::default(),
                                            lights: LightEntities::new(),
                                        };

                                        stored_entities_indexes.shared_sections_ids.insert(shared_section_index);

                                        self.stored_entities_indexes.insert(world_section_id, stored_entities_indexes);
                                    }
                            }

                            // Register the world section with the shared section so that if the shared section is removed,
                            // the world sections pointing to it can be notified
                            self.reverse_shared_section_lookup.entry(shared_section_index).or_insert(Vec::new()).push(world_section_id);

                            // Create the links between related sections. Note: this is done on a per world section, rather
                            // than on a per shared section as when traversing the related section links, each "node" (the world section)
                            // will check its shared section
                            if !self.related_world_sections.contains_key(&world_section_id)
                            {
                                self.related_world_sections.insert(world_section_id, Vec::new());
                                self.register_created_section_with_others(world_section_id);
                            }
                        }
                    }
            }

            self.changed_shared_sections.insert(shared_section_index);
        }
        else
        {
            let world_section_id = BoundingBoxTree::find_unique_world_section_id(bounding_volume.clone(), self.atomic_section_length);

            if self.entity_exists_in_section(entity_id, &WorldSectionLookup::Unique(world_section_id))
            {
                return Ok(());
            }

            if light_type.is_some()
            {
                self.unique_sections_with_lights.insert(world_section_id);
            }

            // Register the world section that the entity is in with the entity
            match self.stored_entities_indexes.get_mut(&world_section_id)
            {
                Some(i) =>
                    {
                        i.lights.add_light_entity(entity_id, light_type);

                        if is_static
                        {
                            i.static_entities.insert(entity_id);

                            // Signal that static entity information for this world section needs to be updated
                            self.changed_static_unique_sections.insert(world_section_id);
                        }
                        else
                        {
                            i.local_entities.insert(entity_id);
                        }

                        match self.changed_world_sections.contains(&world_section_id)
                        {
                            true => self.total_world_aabb_combining += 1,
                            false => self.total_world_aabb_combining += (i.local_entities.len() + i.static_entities.len()) as u32
                        }
                    },
                None =>
                    {
                        let mut stored_entities_indexes = UniqueWorldSectionEntities
                        {
                            aabb: StaticAABB::point_aabb(),
                            back_up_aabb: world_section_id.to_aabb(self.atomic_section_length),
                            local_entities: HashSet::default(),
                            static_entities: HashSet::default(),
                            shared_sections_ids: HashSet::default(),
                            lights: LightEntities::new(),
                        };

                        stored_entities_indexes.lights.add_light_entity(entity_id, light_type);

                        if is_static
                        {
                            stored_entities_indexes.static_entities.insert(entity_id);
                            self.changed_static_unique_sections.insert(world_section_id);
                        }
                        else
                        {
                            stored_entities_indexes.local_entities.insert(entity_id);
                        }

                        self.stored_entities_indexes.insert(world_section_id, stored_entities_indexes);

                        self.total_world_aabb_combining += 1;
                    }
            }

            // Specify where to find the entity given its id
            self.entities_index_lookup.insert(entity_id, WorldSectionLookup::Unique(world_section_id));

            // Create the links between related sections
            if !self.related_world_sections.contains_key(&world_section_id)
            {
                self.related_world_sections.insert(world_section_id, Vec::new());
                self.register_created_section_with_others(world_section_id);
            }

            self.changed_world_sections.insert(world_section_id);
        }

        Ok(())
    }

    fn entity_exists_in_section(&mut self, entity_id: EntityId, section: &WorldSectionLookup) -> bool
    {
        let entity_same_section = if let Some(entity_world_section) = self.entities_index_lookup.get(&entity_id)
        {
            *section == *entity_world_section
        }
        else
        {
            false
        };

        if !entity_same_section
        {
            self.remove_entity(entity_id);
        }

        entity_same_section
    }

    /// Removes the entity from the bounding tree, with appropriate logic to handle if the entity is in
    /// a unique world section or in a shared world section
    ///
    /// `entity_id` - the entity to remove from the tree
    pub fn remove_entity(&mut self, entity_id: EntityId)
    {
        // If the entity was actually added at some point in the past
        if let Some(entity_lookup_key) = self.entities_index_lookup.remove(&entity_id)
        {
            let mut unique_world_sections_to_remove = Vec::new();

            match entity_lookup_key
            {
                WorldSectionLookup::Shared(shared_section_index) =>
                    {
                        let shared_entity_map_empty;

                        // Remove entity from map associated with entities in multiple world sections.
                        // Check if map of entities is now empty- if it is, remove it.
                        // This minimizes chances of a large amount of maps being stored in memory, using a lot of memory

                        match self.shared_section_indexes.get_mut(&shared_section_index)
                        {
                            Some(i) =>
                                {
                                    if i.lights.is_empty()
                                    {
                                        for x in shared_section_index.to_world_sections().iter().filter_map(|x| *x)
                                        {
                                            if i.static_entities.contains(&entity_id)
                                            {
                                                self.changed_static_unique_sections.insert(x);
                                            }

                                            if let Some(unique_section) = self.stored_entities_indexes.get(&x)
                                            {
                                                if unique_section.lights.is_empty()
                                                {
                                                    if !unique_section.is_key_to_shared_section_light(&self.shared_section_lights)
                                                    {
                                                        self.unique_sections_with_lights.remove(&x);
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    i.remove_entity(entity_id);
                                    i.lights.remove_light_entity(entity_id);

                                    shared_entity_map_empty = i.entities.is_empty() && i.static_entities.is_empty();
                                },
                            None => unreachable!()
                        }

                        if shared_entity_map_empty
                        {
                            // Need to notify all world sections pointing to this shared section that it is going
                            // to be deleted, so that they no longer reference it
                            match self.reverse_shared_section_lookup.get(&shared_section_index)
                            {
                                Some(i) =>
                                    {
                                        for world_section in i
                                        {
                                            match self.stored_entities_indexes.get_mut(world_section)
                                            {
                                                Some(j) =>
                                                    {
                                                        j.shared_sections_ids.remove(&shared_section_index);

                                                        if  j.local_entities.is_empty() &&
                                                            j.static_entities.is_empty() &&
                                                            j.shared_sections_ids.is_empty()
                                                        {
                                                            unique_world_sections_to_remove.push(*world_section);
                                                        }
                                                    },
                                                None => unreachable!()
                                            }
                                        }
                                    },
                                None => unreachable!()
                            }

                            self.shared_section_indexes.remove(&shared_section_index);
                            self.reverse_shared_section_lookup.remove(&shared_section_index);
                        }

                        self.changed_shared_sections.insert(shared_section_index);
                    },
                WorldSectionLookup::Unique(world_section_index) =>
                    {

                        // Removes entity from the map associated with entities in a unique world section.
                        // The world section is removed only if it contains no unique and no shared entities-
                        // this is because a world section logically should exist if an entity exists in the world section,
                        // regardless of how much of an entity is contained in the world section

                        match self.stored_entities_indexes.get_mut(&world_section_index)
                        {
                            Some(i) =>
                                {
                                    i.lights.remove_light_entity(entity_id);
                                    if i.lights.is_empty()
                                    {
                                        if !i.is_key_to_shared_section_light(&self.shared_section_lights)
                                        {
                                            self.unique_sections_with_lights.remove(&world_section_index);
                                        }
                                    }

                                    if !i.local_entities.remove(&entity_id)
                                    {
                                        i.static_entities.remove(&entity_id);

                                        // Signal that static entity information for this world section needs to be updated
                                        self.changed_static_unique_sections.insert(world_section_index);
                                    }

                                    if  i.local_entities.is_empty() &&
                                        i.static_entities.is_empty() &&
                                        i.shared_sections_ids.is_empty()
                                    {
                                        unique_world_sections_to_remove.push(world_section_index);
                                    }
                                    else
                                    {
                                        match self.changed_world_sections.contains(&world_section_index)
                                        {
                                            true => self.total_world_aabb_combining += 1,
                                            false => self.total_world_aabb_combining += (i.local_entities.len() + i.static_entities.len()) as u32
                                        }
                                    }
                                },
                            None => unreachable!()
                        }

                        self.changed_world_sections.insert(world_section_index);
                    }
            }

            for world_section in unique_world_sections_to_remove
            {
                let affected_sections = self.related_world_sections.remove(&world_section).unwrap().clone();

                for section in affected_sections
                {
                    let related_sections = self.related_world_sections.get_mut(&section).unwrap();
                    let remove_position = related_sections.iter().position(|x| *x == world_section).unwrap();
                    related_sections.remove(remove_position);
                }

                self.stored_entities_indexes.remove(&world_section);
            }

            // The check if a world section is active also checks for existence of a world section,
            // so removing world sections here before updating the static world section list does not matter
        }
    }

    /// Get all of the entities that are either in the given world sections, or in the shared sections
    /// that are made of the given world sections
    ///
    /// `affected_world_sections` - the world sections to find entities for
    /// `logic_culler` - structure to decide if shared sections that are not visible have their entities included.
    /// `render_culler` - additional structure to that provides a second visibility AABB check that is ORed with the logic culler result
    pub fn find_related_entities<T: TraversalDecider, U: TraversalDecider>(&self, affected_world_section: Vec<UniqueWorldSectionId>, logic_culler: &T, render_culler: &U) -> Vec<RelatedEntitySearchResult>
    {
        self.find_related_entities_internal(affected_world_section, Some(logic_culler), Some(render_culler))
    }

    /// Helper function for find_related entities
    ///
    /// `affected_world_sections` - the world sections to find entities for
    /// `logic_culler` - structure to decide if shared sections that are not visible have their entities included.
    /// `render_culler` - additional structure to that provides a second visibility AABB check that is ORed with the logic culler result
    fn find_related_entities_internal<T: TraversalDecider, U: TraversalDecider>(&self, mut affected_world_section: Vec<UniqueWorldSectionId>, logic_culler: Option<&T>, render_culler: Option<&U>) -> Vec<RelatedEntitySearchResult>
    {
        let mut search_results = Vec::new();

        let mut processed_world_sections: HashSet<UniqueWorldSectionId> = HashSet::default();

        let mut processed_shared_sections: HashSet<SharedWorldSectionId> = HashSet::default();

        while let Some(x) = affected_world_section.pop()
        {
            // Prevent infinite loop- two related sections reference each other. This loop would
            // always add the other section, process the other section, add the first section, process the first section,
            // and so on. This check stops that scenario from happening
            if !processed_world_sections.insert(x)
            {
                continue;
            }

            match self.stored_entities_indexes.get(&x)
            {
                Some(i) =>
                    {
                        let local_result = RelatedEntitySearchResult
                        {
                            location: WorldSectionLookup::Unique(x),
                            entities: &i.local_entities,
                            static_entities: &i.static_entities,
                        };

                        // Append reference to map of entities to be returned
                        search_results.push(local_result);

                        for shared_section in &i.shared_sections_ids
                        {
                            // This check is required because two unrelated world sections can point to the
                            // same shared section (if both are on the same but opposite side of a boundary
                            // for a higher level world division)
                            if processed_shared_sections.insert(*shared_section)
                            {
                                match self.shared_section_indexes.get(shared_section)
                                {
                                    Some(shared_i) =>
                                        {
                                            let aabb_in_view = match (logic_culler, render_culler)
                                            {
                                                (Some(l), Some(r)) => l.aabb_in_view(&shared_i.aabb) || r.aabb_in_view(&shared_i.aabb),
                                                (Some(l), _) => l.aabb_in_view(&shared_i.aabb),
                                                (_, Some(r)) => r.aabb_in_view(&shared_i.aabb),
                                                _ => false
                                            };

                                            // See similar call in logic or render flow for reasoning behind this call
                                            if aabb_in_view
                                            {
                                                let shared_result = RelatedEntitySearchResult
                                                {
                                                    location: WorldSectionLookup::Shared(*shared_section),
                                                    entities: &shared_i.entities,
                                                    static_entities: &i.static_entities,
                                                };

                                                search_results.push(shared_result);
                                            }
                                            else
                                            {
                                                let shared_result = RelatedEntitySearchResult
                                                {
                                                    location: WorldSectionLookup::Shared(*shared_section),
                                                    entities: &shared_i.entities,
                                                    static_entities: &i.static_entities,
                                                };

                                                search_results.push(shared_result);
                                            }
                                        },
                                    None => unreachable!()
                                }
                            }
                        }
                    },
                None => unreachable!("The requested world section does not exist: {:?}, \n\n {:?}", x, self.related_world_sections),
            }

            // Make sure to process entities in related world sections
            affected_world_section.extend(self.related_world_sections.get(&x).unwrap());
        }

        search_results
    }

    /// Calculates the an optimal world section surrounding volume that takes into account the number
    /// entities stored within. This allows for more tight volumes around world sections if the entities
    /// within do not fill the entire world section
    ///
    /// `ecs` - state of entities within the system
    pub fn end_of_changes(&mut self, ecs: &ECS)
    {
        self.update_static_world_sections();

        // If there are too many world sections for which this optimization is done, it can be faster
        // to just not do anything. This also depends on the amount of entities within each world section
        // that will be used for optimizations
        let too_many_aabb_combining = self.total_world_aabb_combining > 500;

        // At level 0 (atomic world section length), do not do optimizations if there more than 20
        // entities and there are many world sections to optimize for
        let base_max_number_entities = 20;

        for x in &self.changed_world_sections
        {
            if let Some(mut world_section_info) = self.stored_entities_indexes.get_mut(x)
            {
                // The bigger the world section, the more entities are allowed to be considered for optimizing.
                // This is because there are less world sections at higher levels
                let adjusted_max_number_entities = (base_max_number_entities + x.level * 5).min(50);

                if too_many_aabb_combining &&
                    (world_section_info.local_entities.len() + world_section_info.static_entities.len()) > adjusted_max_number_entities as usize
                {
                    world_section_info.aabb = world_section_info.back_up_aabb;
                }
                else
                {
                    let mut updated_aabb = StaticAABB::point_aabb();
                    let mut first_entity = true;

                    for entity in world_section_info.local_entities.iter().chain(world_section_info.static_entities.iter())
                    {
                        if first_entity
                        {
                            updated_aabb = ecs.get_copy::<StaticAABB>(*entity).unwrap();
                            first_entity = false;
                            continue;
                        }

                        updated_aabb = updated_aabb.combine_aabb(ecs.get_ref::<StaticAABB>(*entity).unwrap());
                    }

                    world_section_info.aabb = updated_aabb;
                }
            }
        }

        // Same idea as unique world section
        for x in &self.changed_shared_sections
        {
            if let Some(mut world_section_info) = self.shared_section_indexes.get_mut(x)
            {
                let mut updated_aabb = StaticAABB::point_aabb();
                let mut first_entity = true;

                for entity in world_section_info.entities.iter().chain(world_section_info.static_entities.iter())
                {
                    if first_entity
                    {
                        updated_aabb = ecs.get_copy::<StaticAABB>(*entity).unwrap();
                        first_entity = true;
                        continue;
                    }

                    updated_aabb = updated_aabb.combine_aabb(ecs.get_ref::<StaticAABB>(*entity).unwrap());
                }

                world_section_info.aabb = updated_aabb;
            }
        }

        self.changed_shared_sections.clear();
        self.changed_world_sections.clear();
        self.total_world_aabb_combining = 0;
    }

    /// Finds any changes to which world sections contain active entities
    fn update_static_world_sections(&mut self)
    {
        for x in &self.changed_world_sections
        {
            let mut add_static_world_section = false;

            if let Some(i) = self.stored_entities_indexes.get(x)
            {
                if i.local_entities.is_empty()
                {
                    let shared_sections = &i.shared_sections_ids;

                    if shared_sections.is_empty()
                    {
                        add_static_world_section = true;
                    }
                    else
                    {
                        for s in &i.shared_sections_ids
                        {
                            if let Some(j) = self.shared_section_indexes.get(&s)
                            {
                                // Unique world sections are keys/paths to shared world sections, so
                                // even if the section itself has no active entities but a shared section
                                // it is linked to is active, then this section must be considered active
                                if j.entities.is_empty()
                                {
                                    add_static_world_section = true;
                                }
                            }
                        }
                    }
                }
            }

            if add_static_world_section
            {
                self.static_world_sections.insert(*x);
            }
            else
            {
                self.static_world_sections.remove(x);
            }
        }

        for x in &self.changed_shared_sections
        {
            if let Some(i) = self.shared_section_indexes.get(x)
            {
                if i.entities.is_empty()
                {
                    // Unique world sections have their static section status affected as they are the "keys" to
                    // accessing shared section when iterating over visible world sections, hence iterate over unique sections
                    for section in x.to_world_sections().iter().filter_map(|x| *x)
                    {
                        if let Some(world_section) = self.stored_entities_indexes.get(&section)
                        {
                            if world_section.local_entities.is_empty()
                            {
                                self.static_world_sections.insert(section);
                            }
                        }
                    }
                }
                else
                {
                    // There is at least an active entity in this shared section, so all unique world sections
                    // that link with the shared section must be marked as not-static so that this shared section
                    // active entity is processed
                    for section in x.to_world_sections().iter().filter_map(|x| *x)
                    {
                        self.static_world_sections.remove(&section);
                    }
                }
            }

            // If shared section was deleted, leave unique world section that are keys to the shared section
            // as static. Less processing to do in terms of world sections. If active entity is added to that
            // unique world section, then later it will be removed from the static list
        }
    }

    /// Finds all of the world section with entities that are either a parent or a child section of the
    /// given world section
    ///
    /// `created_world_section` - the index of the world section that was created
    fn register_created_section_with_others(&mut self, created_world_section: UniqueWorldSectionId)
    {
        /*
            Note: this function only looks at vertical relationships. For example:

                                            0
                                         ___|___
                                         |      |
                                         1      2

             If world section 1 is passed in, all children of 1 will be looked at, and the parent
             world section 0 is looked at. World section 2, even though it is the child of the parent 0,
             is not looked at.
         */

        // Find all child world sections with entities
        if created_world_section.level != 0
        {
            let mut lower_sections = Vec::from(created_world_section.lower_level_world_section().unwrap());

            while let Some(lower_section) = lower_sections.pop()
            {
                // Remember passed in world section is added to related world section map when entity was added
                match self.related_world_sections.get_mut(&lower_section)
                {
                    Some(i) =>
                        {
                            // Register the created world section with the child world section
                            i.push(created_world_section);

                            // Register the child world section with the created world section
                            self.related_world_sections.get_mut(&created_world_section).unwrap().push(lower_section);
                        },

                    // The child entity does not exist because no entity is within the world section.
                    // That is not an error
                    _ => {}
                }

                // Continue down the tree- just something to note: it is possible that related world
                // sections can skip a level in the tree (if a particular level does not have entities
                // within it, but lower levels do)
                if let Some(lower_levels_to_check) = lower_section.lower_level_world_section()
                {
                    lower_sections.extend(&lower_levels_to_check);
                }
            }
        }

        // Find all parent world section with entities. The logic here is the same as finding child world sections
        if created_world_section.level != self.max_level()
        {
            let mut higher_sections = vec![created_world_section.higher_level_world_section(self.max_level()).unwrap()];

            while let Some(higher_section) = higher_sections.pop()
            {
                match self.related_world_sections.get_mut(&higher_section)
                {
                    Some(i) =>
                        {
                            i.push(created_world_section);
                            self.related_world_sections.get_mut(&created_world_section).unwrap().push(higher_section);
                        },
                    _ => {}
                }

                if let Some(higher_level_to_check) = higher_section.higher_level_world_section(self.max_level())
                {
                    higher_sections.push(higher_level_to_check);
                }
            }
        }
    }

    /// Find the number of world sections that the given world section takes
    ///
    /// `level_length` - the level_length of a world section the bounding volume is a part of
    /// `bounding_volume` - the bounding volume to find th number of world sections it compromises
    // This function is not strictly needed- it exists so that the code that needs this information is more readable
    fn calculate_number_world_sections_total(level_length: u32, bounding_volumes: &StaticAABB) -> u32
    {
        let (num_x, num_y, num_z) = BoundingBoxTree::calculate_number_world_sections_each_dimension(level_length, bounding_volumes);

        num_x * num_y * num_z
    }

    /// Computes the number of world sections taken by an entity's AABB in each dimension.
    ///
    /// `level_length` - the level_length of a world section the bounding volume is a part of
    /// `bounding_volumes` - the bounding volume of the entity being added to the tree
    ///
    /// ```
    /// let (num_x, num_y, num_z) = self.calculate_number_world_sections(&bounding_volumes);
    /// ```
    fn calculate_number_world_sections_each_dimension(level_length: u32, bounding_volumes: &StaticAABB) -> (u32, u32, u32)
    {
        let calculate_number_world_sections = |mut min: f32, max: f32|
            {
                // Both points are in the same world section. This if statement must be the first
                // thing executed in this closure
                if (min / level_length as f32).trunc() == (max / level_length as f32).trunc()
                {
                    return 1;
                }

                // Calculate starting number of world sections. If the starting point is not aligned along a world
                // section, then the number of world sections is one and the starting point is moved to align with
                // the boundary of a world section
                let mut number_world_sections = if (min / level_length as f32).ceil() > (min / level_length as f32)
                {
                    min = (min / level_length as f32).ceil() * level_length as f32;
                    1
                }
                else
                {
                    0
                };

                // Keep moving the starting point until it is past the end point of the range; at that point
                // the number of world sections will have been executed
                while min < max
                {
                    number_world_sections += 1;
                    min += level_length as f32;
                }

                number_world_sections
            };

        let number_world_sections_x = calculate_number_world_sections(bounding_volumes.x_range.min, bounding_volumes.x_range.max);
        let number_world_sections_y = calculate_number_world_sections(bounding_volumes.y_range.min, bounding_volumes.y_range.max);
        let number_world_sections_z = calculate_number_world_sections(bounding_volumes.z_range.min, bounding_volumes.z_range.max);

        (number_world_sections_x, number_world_sections_y, number_world_sections_z)
    }

    /// Find the maximum level for the tree
    pub fn max_level(&self) -> u16
    {
        (self.outline_length as f32 / self.atomic_section_length as f32).log2().ceil() as u16
    }

    /// Compute the indexes (in each dimension) of the closest point to the origin (0, 0, 0) for each
    /// section of an entity's AABB that is in a unique world section.
    ///
    /// `x` - the index of the entity's AABB section in a unique world section in the x-dimension
    /// `y` - the index of the entity's AABB section in a unique world section in the y-dimension
    /// `z` - the index of the entity's AABB section in a unique world section in the z-dimension
    fn calculate_aabb_section_indexes(x: u32, y: u32, z: u32, bounding_volumes: &StaticAABB, atomic_length: u32) -> (u32, u32, u32)
    {
        let aabb_section_x = bounding_volumes.x_range.min as u32 + atomic_length * x;

        let aabb_section_y = bounding_volumes.y_range.min as u32 + atomic_length * y;

        let aabb_section_z = bounding_volumes.z_range.min as u32 + atomic_length * z;

        // Divide by the world section length to get an index from the origin

        (aabb_section_x / atomic_length, aabb_section_y / atomic_length, aabb_section_z / atomic_length)
    }

    /// Clips the AABB so that it does not expand past valid space defined to be the game world
    ///
    /// `aabb` - the bounding volume to normalize
    /// `tree_outline_length` - length of the game world. Game world should be the same size in all dimensions
    fn normalize_aabb(aabb: &mut StaticAABB, tree_outline_length: f32) -> bool
    {
        let out_of_bounds = aabb_helper_functions::aabb_out_of_bounds(aabb, tree_outline_length);

        aabb.x_range.min = aabb.x_range.min.max(0.0).min(tree_outline_length);
        aabb.y_range.min = aabb.y_range.min.max(0.0).min(tree_outline_length);
        aabb.z_range.min = aabb.z_range.min.max(0.0).min(tree_outline_length);

        aabb.x_range.max = aabb.x_range.max.max(0.0).min(tree_outline_length);
        aabb.y_range.max = aabb.y_range.max.max(0.0).min(tree_outline_length);
        aabb.z_range.max = aabb.z_range.max.max(0.0).min(tree_outline_length);

        out_of_bounds
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use super::super::dimension::range::*;
    use super::super::super::objects::ecs::ECS;
    use std::fmt::Debug;
    use nalgebra_glm::vec3;
    use float_cmp::approx_eq;
    use std::iter::FromIterator;
    use crate::culling::logic_frustum_culler::LogicFrustumCuller;

    const ATOMIC_SECTION_LENGTH: u32 = 32;

    struct StoredSectionInformation
    {
        world_section: UniqueWorldSectionId,
        local_entities: Vec<EntityId>,
        shared_world_sections: Vec<SharedWorldSectionId>,
    }

    struct RelatedSectionInformation
    {
        world_section: UniqueWorldSectionId,
        related_world_world_sections: Vec<UniqueWorldSectionId>,
    }

    struct SharedSectionInformation
    {
        shared_section_id: SharedWorldSectionId,
        entities: Vec<EntityId>,
        referenced_by: Vec<UniqueWorldSectionId>,
    }

    struct EntityInformation
    {
        entity_id: EntityId,
        lookup_info: WorldSectionLookup
    }

    fn small_entity_section() -> StaticAABB
    {
        StaticAABB::new
            (
                XRange::new(0.0, 10.0),
                YRange::new(0.0, 10.0),
                ZRange::new(0.0, 10.0)
            )
    }

    fn medium_entity_section() -> StaticAABB
    {
        StaticAABB::new
            (
                XRange::new(0.0, ATOMIC_SECTION_LENGTH as f32),
                YRange::new(0.0, ATOMIC_SECTION_LENGTH as f32),
                ZRange::new(0.0, ATOMIC_SECTION_LENGTH as f32)
            )
    }

    fn large_entity_section() -> StaticAABB
    {
        StaticAABB::new
            (
                XRange::new(0.0, 2.0 * ATOMIC_SECTION_LENGTH as f32),
                YRange::new(0.0, 2.0 * ATOMIC_SECTION_LENGTH as f32),
                ZRange::new(0.0, 2.0 * ATOMIC_SECTION_LENGTH as f32)
            )
    }

    fn create_tree(section_size: u32, entities: Vec<StaticAABB>) -> (BoundingBoxTree, Vec<EntityId>)
    {
        let mut ecs = ECS::new();
        let mut created_entities = Vec::new();

        let mut bounding_box_tree = BoundingBoxTree::new(256, ATOMIC_SECTION_LENGTH);

        for x in entities
        {
            let entity = ecs.create_entity();
            bounding_box_tree.add_entity(entity, &x, false);
            created_entities.push(entity);
        }

        (bounding_box_tree, created_entities)
    }

    fn print_iterator_contents<A: Debug, T: IntoIterator<Item = A>>(iter: T) -> String
    {
        iter.into_iter()
            .map(|x| format!("{:?}", x))
            .fold("\n".to_string(), |previous_value, new_value| previous_value + &new_value + "\n")
    }

    fn check_world_entity_lookup(tree: &BoundingBoxTree, information: Vec<StoredSectionInformation>)
    {
        assert_eq!(information.len(), tree.stored_entities_indexes.len(),
                   "Expected {} world sections, found {}. \n Stored world sections: {}", information.len(), tree.stored_entities_indexes.len(), print_iterator_contents(tree.stored_entities_indexes.keys()));

        for x in &information
        {
            let stored_entities = match tree.stored_entities_indexes.get(&x.world_section)
            {
                Some(i) => i,
                None => panic!("Failed to find world section {:?} in stored entities indexes.  \n Stored world sections: {}", x.world_section, print_iterator_contents(tree.stored_entities_indexes.keys()))
            };

            assert_eq!(x.local_entities.len(), stored_entities.local_entities.len(),
                       "In world section {:?}, expected {} local entities, found {}. \n Stored local entities: {}", x.world_section, x.local_entities.len(), stored_entities.local_entities.len(), print_iterator_contents(stored_entities.local_entities.iter()));

            for local_entity in &x.local_entities
            {
                assert!(stored_entities.local_entities.contains(local_entity),
                        "In world section {:?}, failed to find entity: {:?}. \n Stored local entities: {}", x.world_section, local_entity, print_iterator_contents(stored_entities.local_entities.iter()));
            }

            assert_eq!(x.shared_world_sections.len(), stored_entities.shared_sections_ids.len(),
                       "In world section {:?}, expected {} shared world section, found {}. \n Stored shared world sections: {}", x.world_section, x.shared_world_sections.len(), stored_entities.shared_sections_ids.len(), print_iterator_contents(stored_entities.shared_sections_ids.iter()));

            for shared_world_section in &x.shared_world_sections
            {
                assert!(stored_entities.shared_sections_ids.contains(shared_world_section),
                        "In world section {:?}, failed to find shared world section: {:?}. \n Stored shared world sections: {}", x.world_section, shared_world_section, print_iterator_contents(stored_entities.shared_sections_ids.iter()));
            }
        }
    }

    fn check_related_world_sections(tree: &BoundingBoxTree, information: Vec<RelatedSectionInformation>)
    {
        assert_eq!(information.len(), tree.related_world_sections.len(),
                   "Expected {} world sections, found {}. \n Stored related world sections: {}", information.len(), tree.related_world_sections.len(), print_iterator_contents(tree.related_world_sections.keys()));

        for x in &information
        {
            let related_world_sections = match tree.related_world_sections.get(&x.world_section)
            {
                Some(i) => i,
                None => panic!("Failed to find world section {:?} in related world section indexes.  \n Stored related world sections: {}", x.world_section, print_iterator_contents(tree.related_world_sections.keys()))
            };

            assert_eq!(x.related_world_world_sections, *related_world_sections,
                       "For world section {:?}, expected related world sections: \n{}\n differs than the actual: \n{}", x.world_section, print_iterator_contents(x.related_world_world_sections.iter()), print_iterator_contents(related_world_sections.iter()));
        }
    }

    fn check_shared_sections(tree: &BoundingBoxTree, information: Vec<SharedSectionInformation>)
    {
        assert_eq!(information.len(), tree.shared_section_indexes.len(),
                   "Expected {} shared sections, found {}. \n Stored shared world sections: {}", information.len(), tree.shared_section_indexes.len(), print_iterator_contents(tree.shared_section_indexes.keys()));

        assert_eq!(information.len(), tree.reverse_shared_section_lookup.len(),
                   "Expected {} reverse-lookup shared sections, found {}. \n Stored reverse shared world sections lookup: {}", information.len(), tree.reverse_shared_section_lookup.len(), print_iterator_contents(tree.reverse_shared_section_lookup.keys()));

        for x in &information
        {
            let shared_sections = match tree.shared_section_indexes.get(&x.shared_section_id)
            {
                Some(i) => i,
                None =>  panic!("Failed to find shared section {:?} in shared section indexes.  \n Stored shared sections: {}", x.shared_section_id, print_iterator_contents(tree.shared_section_indexes.keys()))
            };

            let reverse_shared_lookup = match tree.reverse_shared_section_lookup.get(&x.shared_section_id)
            {
                Some(i) => i,
                None =>  panic!("Failed to find reverse shared section {:?} in reverse shared section lookup. \n Stored reverse shared section lookup: {}", x.shared_section_id, print_iterator_contents(tree.reverse_shared_section_lookup.keys()))
            };

            assert_eq!(x.entities.len(), shared_sections.entities.len(),
                       "In shared section {:?}, expected {} shared entities, found {}. \n Stored shared entities: {}", x.shared_section_id, x.entities.len(), shared_sections.entities.len(), print_iterator_contents(shared_sections.entities.iter()));

            for shared_entity in &x.entities
            {
                assert!(shared_sections.entities.contains(shared_entity),
                        "In shared section {:?}, failed to find entity: {:?}. \n Stored shared entities: {}", x.shared_section_id, shared_entity, print_iterator_contents(shared_sections.entities.iter()));
            }

            assert_eq!(x.referenced_by, *reverse_shared_lookup,
                       "For shared section {:?}, expected referenced world sections: \n{}\n differs than the actual: \n{}", x.shared_section_id, print_iterator_contents(x.referenced_by.iter()), print_iterator_contents(reverse_shared_lookup.iter()));
        }
    }

    fn check_entity_lookup(tree: &BoundingBoxTree, information: Vec<EntityInformation>)
    {
        assert_eq!(information.len(), tree.entities_index_lookup.len(),
                   "Expected {} entities, found {}. \n Stored entities: {}", information.len(), tree.entities_index_lookup.len(), print_iterator_contents(tree.entities_index_lookup.iter()));

        for x in &information
        {
            let entity_lookup = match tree.entities_index_lookup.get(&x.entity_id)
            {
                Some(i) => i,
                None => panic!("Failed to find entity {:?} in tree. Stored entities: {}", x.entity_id, print_iterator_contents(tree.entities_index_lookup.iter()))
            };

            assert_eq!(x.lookup_info, *entity_lookup,
                       "Different entity lookup information for {:?}. Expected: {:?}, found: {:?}", x.entity_id, x.lookup_info, entity_lookup);
        }
    }

    // Verify invariants where two entities in a lowest level unique world section is added.
    // One of the entities is smaller than a lowest level unique world section; the other one is of the same size
    #[test]
    fn add_smaller_equal_entity_world_size()
    {
        let smaller_aabb = small_entity_section();
        let medium_aabb = medium_entity_section();

        let (tree, entities) = create_tree(256, vec![smaller_aabb, medium_aabb]);

        let stored_section_information = StoredSectionInformation
        {
            world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
            local_entities: entities.clone(),
            shared_world_sections: Vec::new()
        };

        let related_section_information = RelatedSectionInformation
        {
            world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
            related_world_world_sections: vec![]
        };

        let entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Unique(UniqueWorldSectionId::new(0, 0, 0, 0))
                },

                EntityInformation
                {
                    entity_id: entities[1],
                    lookup_info: WorldSectionLookup::Unique(UniqueWorldSectionId::new(0, 0, 0, 0))
                },
            ];

        check_world_entity_lookup(&tree, vec![stored_section_information]);
        check_related_world_sections(&tree, vec![related_section_information]);
        check_shared_sections(&tree, vec![]);
        check_entity_lookup(&tree, entity_information);
    }

    // Verify invariants when a single entity in the second-lowest level unique world section is added.
    // The entity is equal to the size of a unique world section in the second-lowest level
    #[test]
    fn add_larger_entity_world_size()
    {
        let (tree, entities) = create_tree(256, vec![large_entity_section()]);

        let stored_section_information = StoredSectionInformation
        {
            world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
            local_entities: entities.clone(),
            shared_world_sections: Vec::new()
        };

        let related_section_information = RelatedSectionInformation
        {
            world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
            related_world_world_sections: vec![]
        };

        let entity_information = EntityInformation
        {
            entity_id: entities[0],
            lookup_info: WorldSectionLookup::Unique(UniqueWorldSectionId::new(1, 0, 0, 0))
        };

        check_world_entity_lookup(&tree, vec![stored_section_information]);
        check_related_world_sections(&tree, vec![related_section_information]);
        check_shared_sections(&tree, vec![]);
        check_entity_lookup(&tree, vec![entity_information]);
    }

    // Verify invariants where two entities with an offset in a lowest level unique world section is added.
    // One of the entities is smaller than a lowest level unique world section; the other one is of the same size
    #[test]
    fn add_smaller_equal_entity_offset()
    {
        let mut smaller_aabb = small_entity_section();
        smaller_aabb.translate(vec3(30.0, 0.0, 0.0));

        let mut medium_aabb = medium_entity_section();
        medium_aabb.translate(vec3(5.0, 0.0, 0.0));

        let (tree, entities) = create_tree(256, vec![smaller_aabb, medium_aabb]);

        // Combination of all world section that share this shared section
        let shared_section = SharedWorldSectionId::new(&vec![UniqueWorldSectionId::new(0, 0, 0, 0), UniqueWorldSectionId::new(0, 1, 0, 0)]);

        let stored_section_information =
            vec!
            [
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    local_entities: vec![],
                    shared_world_sections: vec![shared_section]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 1, 0, 0),
                    local_entities: vec![],
                    shared_world_sections: vec![shared_section]
                },
            ];

        let related_section_information =
            vec!
            [
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    related_world_world_sections: vec![]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 1, 0, 0),
                    related_world_world_sections: vec![]
                },
            ];

        let shared_section_info = SharedSectionInformation
        {
            shared_section_id: shared_section,
            entities: entities.clone(),
            referenced_by: vec![UniqueWorldSectionId::new(0, 0, 0, 0), UniqueWorldSectionId::new(0, 1, 0, 0)]
        };

        let entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Shared(shared_section)
                },

                EntityInformation
                {
                    entity_id: entities[1],
                    lookup_info: WorldSectionLookup::Shared(shared_section)
                }
            ];

        check_world_entity_lookup(&tree, stored_section_information);
        check_related_world_sections(&tree, related_section_information);
        check_shared_sections(&tree, vec![shared_section_info]);
        check_entity_lookup(&tree, entity_information);
    }

    // Verify invariants when a single entity with an offset in the second-lowest level unique world section is added.
    // The entity is equal to the size of a unique world section in the second-lowest level
    #[test]
    fn add_large_entity_offset()
    {
        let mut large_aabb = large_entity_section();
        large_aabb.translate(vec3(5.0, 0.0, 0.0));

        let (tree, entities) = create_tree(256, vec![large_aabb]);

        // Combination of all world section that share this shared section
        let shared_section = SharedWorldSectionId::new(&vec![UniqueWorldSectionId::new(1, 0, 0, 0), UniqueWorldSectionId::new(1, 1, 0, 0)]);

        let stored_section_information =
            vec!
            [
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    local_entities: vec![],
                    shared_world_sections: vec![shared_section]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    local_entities: vec![],
                    shared_world_sections: vec![shared_section]
                },
            ];

        let related_section_information =
            vec!
            [
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    related_world_world_sections: vec![]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    related_world_world_sections: vec![]
                },
            ];

        let shared_section_info = SharedSectionInformation
        {
            shared_section_id: shared_section,
            entities: entities.clone(),
            referenced_by: vec![UniqueWorldSectionId::new(1, 0, 0, 0), UniqueWorldSectionId::new(1, 1, 0, 0)]
        };

        let entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Shared(shared_section)
                },
            ];

        check_world_entity_lookup(&tree, stored_section_information);
        check_related_world_sections(&tree, related_section_information);
        check_shared_sections(&tree, vec![shared_section_info]);
        check_entity_lookup(&tree, entity_information);
    }

    #[test]
    pub fn section_relationship()
    {
        let (tree, entities) = create_relationship_tree(vec![], true);
        test_relationship_tree(&tree, &entities, vec![]);

        let (tree, mut entities) = create_relationship_tree(vec![], false);

        let temp = entities[0];
        entities[0] = entities[1];
        entities[1] = temp;

        test_relationship_tree(&tree, &entities, vec![]);
    }

    #[test]
    pub fn remove_entity_shared_entities()
    {
        let mut offset_large_abb = large_entity_section();
        offset_large_abb.translate(vec3(5.0, 0.0, 0.0));

        let (mut tree, entities) = create_relationship_tree(vec![offset_large_abb], true);

        // Obtained from test_relationship_tree()
        let shared_section_id = SharedWorldSectionId::new(&vec![UniqueWorldSectionId::new(1, 0, 0, 0), UniqueWorldSectionId::new(1, 1, 0, 0)]);

        let added_entity_info = EntityInformation
        {
            entity_id: entities[4],
            lookup_info: WorldSectionLookup::Shared(shared_section_id)
        };

        test_relationship_tree(&tree, &entities, vec![added_entity_info]);
        tree.remove_entity(entities[4]);
        test_relationship_tree(&tree, &entities, vec![]);
        tree.remove_entity(entities[2]);

        let stored_section_information =
            vec!
            [
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    local_entities: vec![entities[0]],
                    shared_world_sections: vec![]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    local_entities: vec![entities[1]],
                    shared_world_sections: vec![]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    local_entities: vec![entities[3]],
                    shared_world_sections: vec![]
                },
            ];

        let related_section_information =
            vec!
            [
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    related_world_world_sections: vec![UniqueWorldSectionId::new(1, 0, 0, 0)]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    related_world_world_sections: vec![UniqueWorldSectionId::new(0, 0, 0, 0)]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    related_world_world_sections: vec![]
                },
            ];

        let mut entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(0, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[1],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[3],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 1, 0, 0) )
                },
            ];

        check_world_entity_lookup(&tree, stored_section_information);
        check_related_world_sections(&tree, related_section_information);
        check_shared_sections(&tree, vec![]);
        check_entity_lookup(&tree, entity_information);
    }

    #[test]
    fn remove_unique_first_entities()
    {
        let large_aabb = large_entity_section();

        let (mut tree, entities) = create_relationship_tree(vec![large_aabb], true);

        let added_entity_info = EntityInformation
        {
            entity_id: entities[4],
            lookup_info: WorldSectionLookup::Unique(UniqueWorldSectionId::new(1, 0, 0, 0))
        };

        test_relationship_tree(&tree, &entities, vec![added_entity_info]);
        tree.remove_entity(entities[4]);
        test_relationship_tree(&tree, &entities, vec![]);
        tree.remove_entity(entities[1]);

        // Obtained from test_relationship_tree()
        let shared_section_id = SharedWorldSectionId::new(&vec![UniqueWorldSectionId::new(1, 0, 0, 0), UniqueWorldSectionId::new(1, 1, 0, 0)]);

        let mut stored_section_information =
            vec!
            [
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    local_entities: vec![entities[0]],
                    shared_world_sections: vec![]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    local_entities: vec![],
                    shared_world_sections: vec![shared_section_id]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    local_entities: vec![entities[3]],
                    shared_world_sections: vec![shared_section_id]
                },
            ];

        let related_section_information =
            vec!
            [
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    related_world_world_sections: vec![UniqueWorldSectionId::new(1, 0, 0, 0)]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    related_world_world_sections: vec![UniqueWorldSectionId::new(0, 0, 0, 0)]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    related_world_world_sections: vec![]
                },
            ];

        let mut shared_section_info = SharedSectionInformation
        {
            shared_section_id,
            entities: vec![entities[2]],
            referenced_by: vec![UniqueWorldSectionId::new(1, 0, 0, 0), UniqueWorldSectionId::new(1, 1, 0, 0)]
        };

        let mut entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(0, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[2],
                    lookup_info: WorldSectionLookup::Shared(shared_section_id)
                },
                EntityInformation
                {
                    entity_id: entities[3],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 1, 0, 0) )
                },
            ];

        check_world_entity_lookup(&tree, stored_section_information);
        check_related_world_sections(&tree, related_section_information);
        check_shared_sections(&tree, vec![shared_section_info]);
        check_entity_lookup(&tree, entity_information);

        tree.remove_entity(entities[2]);

        let mut stored_section_information =
            vec!
            [
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    local_entities: vec![entities[0]],
                    shared_world_sections: vec![]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    local_entities: vec![entities[3]],
                    shared_world_sections: vec![]
                },
            ];

        let related_section_information =
            vec!
            [
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    related_world_world_sections: vec![]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    related_world_world_sections: vec![]
                },
            ];

        let mut entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(0, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[3],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 1, 0, 0) )
                },
            ];

        check_world_entity_lookup(&tree, stored_section_information);
        check_related_world_sections(&tree, related_section_information);
        check_shared_sections(&tree, vec![]);
        check_entity_lookup(&tree, entity_information);
    }

    #[test]
    fn remove_unique_last_entities()
    {
        let large_aabb = large_entity_section();

        let (mut tree, entities) = create_relationship_tree(vec![large_aabb], true);

        let added_entity_info = EntityInformation
        {
            entity_id: entities[4],
            lookup_info: WorldSectionLookup::Unique(UniqueWorldSectionId::new(1, 0, 0, 0))
        };

        test_relationship_tree(&tree, &entities, vec![added_entity_info]);
        tree.remove_entity(entities[4]);
        test_relationship_tree(&tree, &entities, vec![]);
        tree.remove_entity(entities[2]);

        let mut stored_section_information =
            vec!
            [
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    local_entities: vec![entities[0]],
                    shared_world_sections: vec![]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    local_entities: vec![entities[1]],
                    shared_world_sections: vec![]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    local_entities: vec![entities[3]],
                    shared_world_sections: vec![]
                },
            ];

        let related_section_information =
            vec!
            [
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    related_world_world_sections: vec![UniqueWorldSectionId::new(1, 0, 0, 0)]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    related_world_world_sections: vec![UniqueWorldSectionId::new(0, 0, 0, 0)]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    related_world_world_sections: vec![]
                },
            ];

        let mut entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(0, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[1],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[3],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 1, 0, 0) )
                },
            ];

        check_world_entity_lookup(&tree, stored_section_information);
        check_related_world_sections(&tree, related_section_information);
        check_shared_sections(&tree, vec![]);
        check_entity_lookup(&tree, entity_information);

        tree.remove_entity(entities[1]);

        let mut stored_section_information =
            vec!
            [
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    local_entities: vec![entities[0]],
                    shared_world_sections: vec![]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    local_entities: vec![entities[3]],
                    shared_world_sections: vec![]
                },
            ];

        let related_section_information =
            vec!
            [
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    related_world_world_sections: vec![]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    related_world_world_sections: vec![]
                },
            ];

        let mut entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(0, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[3],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 1, 0, 0) )
                },
            ];

        check_world_entity_lookup(&tree, stored_section_information);
        check_related_world_sections(&tree, related_section_information);
        check_shared_sections(&tree, vec![]);
        check_entity_lookup(&tree, entity_information);
    }

    #[test]
    fn find_related_entities()
    {
        let small_aabb = small_entity_section();
        let large_aabb = large_entity_section();
        let large_aabb2 = large_entity_section();

        let very_large_entity = StaticAABB::new
            (
                XRange::new(0.0, 4.0 * ATOMIC_SECTION_LENGTH as f32),
                YRange::new(0.0, 4.0 * ATOMIC_SECTION_LENGTH as f32),
                ZRange::new(0.0, 4.0 * ATOMIC_SECTION_LENGTH as f32),
            );

        let mut offset_large_aabb2 = large_entity_section();
        offset_large_aabb2.translate(vec3(5.0, 0.0, 0.0));

        let mut unrelated_small_aabb = small_entity_section();
        unrelated_small_aabb.translate(vec3(128.0, 0.0, 0.0));

        let (tree, entities) = create_tree(256, vec![small_aabb, large_aabb, large_aabb2, offset_large_aabb2, very_large_entity, unrelated_small_aabb]);

        let entity_maps = vec![
            FnvHashSet::from_iter(vec![entities[0]].into_iter()),
            FnvHashSet::from_iter(vec![entities[1], entities[2]].into_iter()),
            FnvHashSet::from_iter(vec![entities[3]].into_iter()),
            FnvHashSet::from_iter(vec![entities[4]].into_iter()),
            HashSet::default(),
            FnvHashSet::from_iter(vec![entities[5]].into_iter()) // Unrelated entity search result
        ];

        let expected_related_entities = vec![
            RelatedEntitySearchResult
            {
                location: WorldSectionLookup::Unique(UniqueWorldSectionId::new(0, 0, 0, 0)),
                entities: &entity_maps[0]
            },
            RelatedEntitySearchResult
            {
                location: WorldSectionLookup::Unique(UniqueWorldSectionId::new(1, 0, 0, 0)),
                entities: &entity_maps[1]
            },
            RelatedEntitySearchResult
            {
                location: WorldSectionLookup::Shared(SharedWorldSectionId::new(&vec![UniqueWorldSectionId::new(1, 0, 0, 0), UniqueWorldSectionId::new(1, 1, 0, 0)])),
                entities: &entity_maps[2]
            },
            RelatedEntitySearchResult
            {
                location: WorldSectionLookup::Unique(UniqueWorldSectionId::new(2, 0, 0, 0)),
                entities: &entity_maps[3]
            },
            RelatedEntitySearchResult
            {
                location: WorldSectionLookup::Unique(UniqueWorldSectionId::new(1, 1, 0, 0)),
                entities: &entity_maps[4]
            }
        ];

        let expected_unrelated_entities = vec![
            RelatedEntitySearchResult
            {
                location: WorldSectionLookup::Unique(UniqueWorldSectionId::new(0, 4, 0, 0)),
                entities: &entity_maps[5]
            }
        ];

        let check_expected_entities = |aabb: StaticAABB, expected_entities: &Vec<RelatedEntitySearchResult>|
            {
                let mut actual_related_entities = tree.find_related_entities_internal::<LogicFrustumCuller>(tree.find_all_unique_world_section_ids(&aabb), None);

                for search_result in expected_entities
                {
                    let found_result = actual_related_entities.iter().find(|x| **x == *search_result ).is_some();
                    assert!(found_result, "Unable to find search result {:?} with AABB {:?}", search_result, aabb);
                }
            };

        check_expected_entities(small_aabb, &expected_related_entities);
        check_expected_entities(large_aabb, &expected_related_entities);
        check_expected_entities(large_aabb2, &expected_related_entities);
        check_expected_entities(offset_large_aabb2, &expected_related_entities);

        check_expected_entities(unrelated_small_aabb, &expected_unrelated_entities);
    }

    #[test]
    fn world_section_to_aabb()
    {
        let world_id = UniqueWorldSectionId::new(1, 3, 1, 2);
        let static_aabb = world_id.to_aabb(32);

        assert!( approx_eq!(f32, static_aabb.x_range.min, 192.0, ulps = 2));
        assert!( approx_eq!(f32, static_aabb.x_range.max, 256.0, ulps = 2));

        assert!( approx_eq!(f32, static_aabb.z_range.min, 64.0, ulps = 2));
        assert!( approx_eq!(f32, static_aabb.z_range.max, 128.0, ulps = 2));

        assert!( approx_eq!(f32, static_aabb.y_range.min, 128.0, ulps = 2));
        assert!( approx_eq!(f32, static_aabb.y_range.max, 192.0, ulps = 2));
    }

    fn create_relationship_tree(additional_aabbs: Vec<StaticAABB>, add_smaller_entities_first: bool) -> (BoundingBoxTree, Vec<EntityId>)
    {
        // These will be in a relationship
        let small_aabb = small_entity_section();
        let large_abb = large_entity_section();
        let mut offset_large_abb = large_entity_section();
        offset_large_abb.translate(vec3(5.0, 0.0, 0.0));

        // This will not be in the relationship
        let mut neighbor_aabb = large_entity_section();
        neighbor_aabb.translate(vec3(2.0 * ATOMIC_SECTION_LENGTH as f32, 0.0, 0.0));

        let mut aabb_vec = if add_smaller_entities_first
        {
            vec![small_aabb, large_abb, offset_large_abb, neighbor_aabb]
        }
        else
        {
            vec![large_abb, small_aabb, offset_large_abb, neighbor_aabb]
        };

        aabb_vec.extend(additional_aabbs.iter());

        create_tree(256, aabb_vec)
    }

    fn test_relationship_tree(tree: &BoundingBoxTree, entities: &Vec<EntityId>, append_entities: Vec<EntityInformation>)
    {
        let shared_section_id = SharedWorldSectionId::new(&vec![UniqueWorldSectionId::new(1, 0, 0, 0), UniqueWorldSectionId::new(1, 1, 0, 0)]);

        let mut stored_section_information =
            vec!
            [
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    local_entities: vec![entities[0]],
                    shared_world_sections: vec![]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    local_entities: vec![entities[1]],
                    shared_world_sections: vec![shared_section_id]
                },
                StoredSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    local_entities: vec![entities[3]],
                    shared_world_sections: vec![shared_section_id]
                },
            ];

        let related_section_information =
            vec!
            [
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(0, 0, 0, 0),
                    related_world_world_sections: vec![UniqueWorldSectionId::new(1, 0, 0, 0)]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 0, 0, 0),
                    related_world_world_sections: vec![UniqueWorldSectionId::new(0, 0, 0, 0)]
                },
                RelatedSectionInformation
                {
                    world_section: UniqueWorldSectionId::new(1, 1, 0, 0),
                    related_world_world_sections: vec![]
                },
            ];

        let mut shared_section_info = SharedSectionInformation
        {
            shared_section_id,
            entities: vec![entities[2]],
            referenced_by: vec![UniqueWorldSectionId::new(1, 0, 0, 0), UniqueWorldSectionId::new(1, 1, 0, 0)]
        };

        let mut entity_information =
            vec!
            [
                EntityInformation
                {
                    entity_id: entities[0],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(0, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[1],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 0, 0, 0) )
                },
                EntityInformation
                {
                    entity_id: entities[2],
                    lookup_info: WorldSectionLookup::Shared(shared_section_id)
                },
                EntityInformation
                {
                    entity_id: entities[3],
                    lookup_info: WorldSectionLookup::Unique( UniqueWorldSectionId::new(1, 1, 0, 0) )
                },
            ];

        for x in &append_entities
        {
            match x.lookup_info
            {
                WorldSectionLookup::Shared(ref i) => { shared_section_info.entities.push(x.entity_id); },
                WorldSectionLookup::Unique(ref i) =>
                    {
                        let mut world_section = stored_section_information.iter_mut().find(|x| x.world_section == *i).unwrap();

                        world_section.local_entities.push(x.entity_id);
                    }
            }
        }

        entity_information.extend(append_entities.into_iter());

        check_world_entity_lookup(tree, stored_section_information);
        check_related_world_sections(tree, related_section_information);
        check_shared_sections(tree, vec![shared_section_info]);
        check_entity_lookup(tree, entity_information);

    }
}