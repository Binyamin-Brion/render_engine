use std::any::TypeId;
use std::collections::BTreeSet;
use std::ptr::copy_nonoverlapping;
use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::exports::light_components::{DirectionLight, PointLight, SpotLight};
use crate::models::model_definitions::ModelId;
use crate::objects::entity_enforcers::ForceCreationEntity;
use crate::objects::entity_id::{EntityId, EntityIdRead};
use crate::objects::sorted_entities::EntityComponentOrganizer;

const COMPONENTS_PER_BYTE: usize = 8;

/// Determines the number of bytes required to store N components at compile time
const fn num_bytes_for_components(number: usize) -> usize
{
    // One bit represents one component
    let components_per_byte = 8;

    // At time of writing, if and loops are not allowed in const functions. Thus this determines if a number
    // is a multiple of 8 using bit operations. Works off of the fact that if a number is not a multiple of 8,
    // then one of the least significant three bits will be a one
    let roundup =   (number & 0x00000001) >> 1 |
        (number & 0x00000002) >> 2 |
        (number & 0x00000003) >> 3 ;

    // If the number is not a multiple of 8, then an additional byte needs to be added as the
    // division will otherwise return one byte less than what is required
    number / components_per_byte + roundup
}

/// Computes the specific byte and bit that represents the given component index
///
/// `component_index` - the index of the component in the ECS for which to calculate the access information
///
/// ```
/// let (byte, bit) = calculate_byte_bit_offset(8); // Byte 1, bit 0 (offset starts at 0!)
/// let component_written = (bitset_byte_array[byte] >> bit) & 0x1;
/// ```
fn calculate_byte_bit_offset(component_index: usize) -> (usize, usize)
{
    let byte = component_index / COMPONENTS_PER_BYTE;

    (byte, component_index - byte * COMPONENTS_PER_BYTE)
}

// This object works by storing a bitset for each entity, where each bit represents a unique component.
// The component a bit represents is determined based off of the respective index in self.registered_types.
// For example, if the self.registered_types[1] = someStruct, then bit 1 for a bitset represents someStruct.
// If a component has been written for an entity, then the respective bit is 1, otherwise 0

// Deleted entities are kept track of, and the bitsets used for those deleted entities are reused
// when creating new entities

// TODO: Parallelize disjoint writes

const MAX_NUMBER_COMPONENTS: usize = 32;

/// An entity-component system. Stores all of the various components types and their values for an entity
#[derive(Clone, Serialize, Deserialize)]
pub struct ECS
{
    registered_types: Vec<IndexInformation>,
    bitsets: Vec<[u8; num_bytes_for_components(MAX_NUMBER_COMPONENTS)]>,
    entity_model_lookup: HashMap<TypeIdentifier, HashSet::<EntityId>>,
    free_indexes: Vec<usize>,
    organizer: EntityComponentOrganizer,
    max_num_components: usize,
    user_entity_id: EntityId,
    owned_entities: HashMap<EntityId, HashSet<EntityId>>,
    referenced_entities: HashMap<EntityId, HashSet<EntityIdRead>>
}

// Stores the actual values of components. To store all of these in the same vector in self.registered_types,
// all of the values are serialized into bytes. The index into this byte array for an entity is stored using
// a hashmap. The index is in bytes, NOT in type 'T'

// Deleted values are kept track of, and those free chunks of bytes are used for the next value that is written

/// Stores the value of components and keeps track which value type 'T' is being stored
#[derive(Clone, Serialize, Deserialize)]
struct IndexInformation
{
    type_id: TypeIdentifier,
    instances: Vec<u8>,
    free_space: Vec<isize>,
    sparse_map: HashMap<EntityId, isize>,
}

/// Serializable version of the standard library TypeId
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypeIdentifier
{
    t: [u64; 1]
}

impl From<TypeId> for TypeIdentifier
{
    fn from(type_id: TypeId) -> Self
    {
        let type_identifier = TypeIdentifier{ t: [0] };

        unsafe
            {
                *(type_identifier.t.as_ptr() as *mut TypeId) = type_id;
            }

        type_identifier
    }
}

impl ECS
{
    /// Creates a new Entity Component System
    ///
    /// ```
    /// let ecs = ECS::new();
    /// ```
    pub fn new() -> ECS
    {
        let default_sortable_components = vec!
        [
            TypeIdentifier::from(TypeId::of::<DirectionLight>()),
            TypeIdentifier::from(TypeId::of::<PointLight>()),
            TypeIdentifier::from(TypeId::of::<SpotLight>()),
        ];

        let mut ecs = ECS
        {
            registered_types: Vec::new(),
            bitsets: Vec::new(),
            entity_model_lookup: HashMap::default(),
            free_indexes: Vec::new(),
            organizer: EntityComponentOrganizer::new(default_sortable_components),
            max_num_components: MAX_NUMBER_COMPONENTS,
            user_entity_id: ECS::get_temporary_entity_id(),
            owned_entities: HashMap::default(),
            referenced_entities: HashMap::default()
        };
        ecs.register_type::<TypeIdentifier>();
        ecs.user_entity_id = ecs.create_entity();
        ecs
    }

    pub fn get_owned_entities(&self, owning: EntityId) -> Option<&HashSet<EntityId>>
    {
        self.owned_entities.get(&owning)
    }

    pub fn get_referenced_entities(&self, owning: EntityId) -> Option<&HashSet<EntityIdRead>>
    {
        self.referenced_entities.get(&owning)
    }

    pub fn add_owned_entity(&mut self, owning: EntityId, other: EntityId)
    {
        let map = self.owned_entities.entry(owning).or_insert(HashSet::default());
        map.insert(other);
    }

    pub fn remove_owned_entity(&mut self, owning: EntityId, other: EntityId)
    {
        if let Some(map) = self.owned_entities.get_mut(&owning)
        {
            map.remove(&other);
        }
    }

    pub fn add_referenced_entity(&mut self, owning: EntityId, other: EntityIdRead)
    {
        let map = self.referenced_entities.entry(owning).or_insert(HashSet::default());
        map.insert(other);
    }

    pub fn remove_referenced_entity(&mut self, owning: EntityId, other: EntityIdRead)
    {
        if let Some(map) = self.referenced_entities.get_mut(&owning)
        {
            map.remove(&other);
        }
    }

    pub fn get_user_id_read(&self) -> EntityIdRead
    {
        EntityIdRead::new(self.user_entity_id)
    }

    pub fn get_user_id(&mut self) -> EntityId
    {
        self.user_entity_id
    }

    pub fn get_temporary_entity_id() -> EntityId
    {
        EntityId::new(u32::MAX, ForceCreationEntity)
    }

    /// Mark the given entity with a sortable component
    ///
    /// `entity_id` - the ID of the entity to add the sortable component to
    /// `component` - the sortable component the entity should have
    pub fn write_sortable_component(&mut self, entity_id: EntityId, component: TypeIdentifier)
    {
        self.organizer.add_entity(entity_id, component);
    }

    /// Remove the sortable component of the given entity, replacing it with the default one
    ///
    /// `entity_id` - the ID of the entity to have its sortable component removed
    pub fn remove_sortable_component(&mut self, entity_id: EntityId)
    {
        self.organizer.add_entity_default_component(entity_id);
    }

    /// Get the number of sortable components that have been registered
    pub fn number_sortable_components(&self) -> usize
    {
        self.organizer.number_sortable_components()
    }

    /// Get all of the entities, and group them together according to the sortable component
    pub fn get_entities_with_sortable(&self) -> Vec<&HashSet::<EntityId>>
    {
        self.organizer.get_entities_with_sortable_components()
    }

    /// Gets the entities that have the given components written for them
    ///
    /// This function will panic if a component that was not registered is passed as a parameter
    ///
    /// `components` - list of components that the returned entities must have attached to them
    ///
    /// ```
    ///  struct Position(u32);
    ///  struct Velocity(u32);
    ///  let entities = ecs.get_indexes_for_components(&[TypeId::of::<Position>(), TypeId::of::<Velocity>()]);
    /// ```
    pub fn get_indexes_for_components(&self, components: &[TypeIdentifier]) -> BTreeSet<EntityId>
    {
        // Find the desired component with the lowest number of entities- Starting with a desired
        // component that has the least number of entities reduces the number of checks that have
        // to be in the other component entities set.
        let index = self.registered_types.iter().position(|x| x.type_id == components[0]).unwrap();
        let mut component_smallest_entities = &self.registered_types[index];

        for component in components.iter().skip(1)
        {
            let index = self.registered_types.iter().position(|x| x.type_id == *component).unwrap();

            if self.registered_types[index].sparse_map.len() < component_smallest_entities.sparse_map.len()
            {
                component_smallest_entities = &self.registered_types[index];
            }
        }

        // Iterate over all of the desired components, and see which entities are continuously in the
        // desired set of components- these entities are the ones that have all of the desired components.

        let mut smallest_entity_set: BTreeSet<EntityId> = component_smallest_entities.sparse_map.keys().cloned().collect();

        for component in components
        {
            // Prevent a potentially large redundant check
            if *component == component_smallest_entities.type_id
            {
                continue;
            }

            let mut refined_smallest_entity_set = BTreeSet::new();

            let index = self.registered_types.iter().position(|x| x.type_id == *component).unwrap();

            for entity_id in smallest_entity_set
            {
                if self.registered_types[index].sparse_map.get(&entity_id).is_some()
                {
                    refined_smallest_entity_set.insert(entity_id);
                }
            }

            smallest_entity_set = refined_smallest_entity_set;
        }

        smallest_entity_set
    }

    /// Get the entities that have the passed in Marker
    ///
    /// `marker` - the marker that the returned entity IDs should have
    ///
    pub fn get_entities_with_type(&self, entity_type: TypeIdentifier) -> BTreeSet<EntityId>
    {
        let create_btree_from_set = |set: &HashSet::<EntityId>|
            {
                let mut tree = BTreeSet::new();

                for entity in set
                {
                    tree.insert(*entity);
                }

                tree
            };

        create_btree_from_set(self.entity_model_lookup.get(&entity_type).unwrap())
    }

    /// Register a type as a component. Afterwards, that component can be added to an entity
    ///
    /// ```
    ///  struct Position;
    ///  ecs.register_type::<Position>();
    /// ```
    pub fn register_type<'a, T: 'static + Serialize + Deserialize<'a>>(&mut self)
    {
        if self.registered_types.len() == MAX_NUMBER_COMPONENTS
        {
            panic!("Instance of ECS can only hold {} components", MAX_NUMBER_COMPONENTS);
        }

        if self.index_of::<T>().is_none()
        {
            self.registered_types.push(IndexInformation::new(TypeIdentifier::from(TypeId::of::<T>())));

            return;
        }

        // If the component is already registered, there is no harm in not doing anything,
        // but probably better to make the issue visible

        println!("The type {:?} was already registered", TypeId::of::<T>());
    }

    /// Checks if a component for an entity exists, which is true if that component has been written
    /// for the given entity
    ///
    /// Result of Ok(true) indicates component is written for the entity.
    /// Result of Ok(false) indicates component is not written for the entity.
    /// Result of Err(()) indicates the type of component was never registered
    ///
    /// `entity_id` - the id of the entity for which to check if a component exists
    ///
    /// ```
    ///  let component_index = 0;
    ///  let entity_id= 1;
    ///  let component_exists = self.check_component_exists_for_object(entity_id, component_index).unwrap() == true;
    /// ```
    pub fn check_component_written<T: 'static>(&self, entity_id: EntityId) -> Result<bool, ()>
    {
        if let Some(component_index) = self.index_of::<T>()
        {
            let component_indexing_information = calculate_byte_bit_offset(component_index);

            let bitset_ptr = &self.bitsets[entity_id.get_entity_instance() as usize][component_indexing_information.0];

            if (*bitset_ptr >> component_indexing_information.1) & 0x1 == 1
            {
                return Ok(true);
            }
            else
            {
                return Ok(false);
            }
        }

        Err(())
    }

    pub fn check_entity_type_written(&self, entity_id: EntityId) -> Result<bool, ()>
    {
        self.check_component_written::<TypeIdentifier>(entity_id)
    }

    pub fn check_component_written_assume_registered<T: 'static>(&self, entity_id: EntityId) -> bool
    {
        self.check_component_written::<T>(entity_id).unwrap()
    }

    /// Creates an entity with no components attached
    ///
    /// ```
    ///  let entity = ecs.create_entity();
    /// ```
    pub fn create_entity(&mut self) -> EntityId
    {
        let entity_id = match self.free_indexes.pop()
        {
            // Reuse an existing bitset if there is one that is not used
            Some(index) => EntityId::new(index as u32, ForceCreationEntity),
            None =>
                {
                    let entity_id = self.bitsets.len() as u32;

                    self.bitsets.push([0; num_bytes_for_components(32)]);

                    EntityId::new(entity_id, ForceCreationEntity)
                }
        };

        self.organizer.add_entity_default_component(entity_id);
        entity_id
    }

    /// Get the index of the component in the appropriate vector [holding the component].
    ///
    /// If the entity does not have the component, or if the component was not registered, None is returned
    ///
    /// `entity_id` - the Id of the entity for which to get the index of a component
    ///
    /// ```
    /// struct Position;
    /// let index = ecs.get_index_offset::<Position>(anEntity);
    /// ```
    pub fn get_index_offset<T: 'static>(&self, entity_id: EntityId) -> Option<usize>
    {
        match self.index_of::<T>()
        {
            Some(i) => self.registered_types[i].get_index::<T>(entity_id),
            None => None
        }
    }

    /// Write a value of a component without type safety, through the value's serialized form
    ///
    /// `entity_id` - the ID of the entity to have a component updated
    /// `type_id` - the ID of the component being updated
    /// `value` - the serialized value of the updated component
    pub unsafe fn write_component_serialized(&mut self, entity_id: EntityId, type_id: TypeIdentifier, value: &Vec<u8>)
    {
        if let Some(component_index) = self.registered_types.iter().position(|x| x.type_id == type_id)
        {
            let component_indexing_information = calculate_byte_bit_offset(component_index);

            let bitset_ptr = &mut self.bitsets[entity_id.get_entity_instance() as usize][component_indexing_information.0];

            // Mark the entity as having this type of component attached to it
            *(bitset_ptr) |= 1 << component_indexing_information.1;

            self.registered_types[component_index].write_serialized_data(entity_id, value);
        }
        else
        {
            panic!("The type {:?} was not registered!", type_id);
        }
    }

    /// Adds a component to an entity. Any previous component of the same type, if any, is overwritten
    ///
    /// `entity_id` - the Id of the entity to which to attach the component
    /// `value` - the component value to attach to the entity
    ///
    /// ```
    ///  struct Position(u32);
    ///  let entity_id = 0;
    ///  ecs.write_component::<Position>(entity_id, Position::new(0));
    /// ```
    pub fn write_component<'a, T: 'static + Serialize + Deserialize<'a>>(&mut self, entity_id: EntityId, value: T)
    {
        if let Some(component_index) = self.index_of::<T>()
        {
            let component_indexing_information = calculate_byte_bit_offset(component_index);

            let bitset_ptr = &mut self.bitsets[entity_id.get_entity_instance() as usize][component_indexing_information.0];

            // Mark the entity as having this type of component attached to it
            *(bitset_ptr) |= 1 << component_indexing_information.1;

            self.registered_types[component_index].write_data(entity_id, value);
        }
        else
        {
            panic!("The type {:?} was not registered!", TypeId::of::<T>());
        }
    }

    /// Writers the given component for an entity. Any previous marker, if any, is overwritten
    ///
    /// `entity_id` - the Id of the entity to which to write the marker
    /// `marker` - the marker to attach to the component
    ///
    pub fn write_entity_type(&mut self, entity_id: EntityId, entity_type: TypeIdentifier)
    {
        if let Some(current_type) = self.get_copy::<TypeIdentifier>(entity_id)
        {
            self.entity_model_lookup.get_mut(&current_type).unwrap().remove(&entity_id);
        }

        let map = self.entity_model_lookup.entry(entity_type).or_insert(HashSet::default());
        map.insert(entity_id);

        self.write_component::<TypeIdentifier>(entity_id, entity_type);
    }

    /// Removes a component from an entity. The entity is still valid after calling this function
    ///
    /// Assumes the caller checked if the component was written for the entity; otherwise this function
    /// has no effect.
    ///
    /// `entity_id` - the Id of the entity to which to delete the component
    ///
    /// ```
    ///  struct Position;
    ///  let entity_id = 4;
    ///  ecs.remove_data::<Position>(entity_id);
    /// ```
    pub fn remove_component<T: 'static>(&mut self, entity_id: EntityId)
    {
        if let Some(component_index) = self.index_of::<T>()
        {
            self.remove_component_internal(entity_id, component_index);
        }
    }

    /// Removes the given component from an entity. The entity is still valid after calling this function
    ///
    /// Assumes the caller checked if the component was written for the entity; otherwise this function
    /// has no effect.
    ///
    /// `entity_id` - the Id of the entity to which to delete the component
    /// `type_id` - the ID of the component to remove
    pub fn remove_component_type_id_internal(&mut self, entity_id: EntityId, type_id: TypeIdentifier)
    {
        if let Some(component_index) =  self.registered_types.iter().position(|x| x.type_id == type_id)
        {
            self.remove_component_internal(entity_id, component_index);
        }
    }

    /// Internal function that both public remove component functions call
    ///
    /// `entity_id` - the ID of the entity to have its component remove
    /// `component_index` - the index of the component to remove
    fn remove_component_internal(&mut self, entity_id: EntityId, component_index: usize)
    {
        let components_per_byte = 8;

        let component_indexing_information = calculate_byte_bit_offset(component_index);

        let bitset_ptr = &mut self.bitsets[entity_id.get_entity_instance() as usize][component_indexing_information.0];

        if (*bitset_ptr >> component_indexing_information.1) & 0x1 == 1
        {
            self.registered_types[component_indexing_information.0 * components_per_byte + component_indexing_information.1].remove_data(entity_id);

            (*bitset_ptr) &= !(1 << component_indexing_information.1);
        }
    }

    /// Removes all components of the entity. The entity afterwards is no longer valid
    ///
    /// `entity_id` - the Id of the entity to delete
    ///
    /// ```
    ///  let entity_id = 1;
    ///  ecs.remove_data(entity_id);
    /// ```
    pub fn remove_entity(&mut self, entity_id: EntityId)
    {
        // If this is called twice for the same entity (and entity id was not reused for a new entity
        // between those deletes), then the free_indexes will contain the same index twice, which could
        // cause two different entities to use the same bitset. This will lead to unexpected behavior
        if self.free_indexes.iter().position(|x| *x as u32 == entity_id.get_entity_instance()).is_some()
        {
            return;
        }

        // Have to iterate over the entire length of the bitset (in bytes) in order to remove all
        // attached components
        for x in 0..num_bytes_for_components(8)
        {
            let components_per_byte = 8;

            for bit in 0..components_per_byte
            {
                // Get a specific byte of a bitset, which represents a subset of the components.
                // If a component was registered for the entity, then remove it.
                if (self.bitsets[entity_id.get_entity_instance() as usize][x] >> bit) & 0x1 == 1
                {
                    self.registered_types[x * 8 + bit].remove_data(entity_id);
                }
            }
        }

        // Easier to just clear the entire bitset, allowing it to be reused for a new entity
        self.bitsets[entity_id.get_entity_instance() as usize] = [0; num_bytes_for_components(32)];

        self.free_indexes.push(entity_id.get_entity_instance() as usize);
    }

    /// Removes the marker for the given entity. If no marker was written for the entity, no action is taken
    ///
    /// `entity_id` - the entity for which to remove its marker
    ///
    pub fn remove_entity_type(&mut self, entity_id: EntityId)
    {
        match self.get_copy::<TypeIdentifier>(entity_id)
        {
            Some(id) =>
                {
                    self.entity_model_lookup.get_mut(&id).unwrap().remove(&entity_id);

                    self.remove_component::<TypeIdentifier>(entity_id);
                },
            None => {},
        }
    }

    pub fn is_entity_empty(&self, entity_id: EntityId) -> bool
    {
        for x in 0..num_bytes_for_components(8)
        {
            let components_per_byte = 8;

            for bit in 0..components_per_byte
            {
                if (self.bitsets[entity_id.get_entity_instance() as usize][x] >> bit) & 0x1 == 1
                {
                    return false;
                }
            }
        }

        return true;
    }

    /// Get a copy of component from a read-only reference to an entity
    ///
    /// `entity_id_read` - the read key of the entity whose component should be read
    pub fn get_copy_read<'a, T: 'static + Copy + Serialize + Deserialize<'a>>(&self, entity_id_read: EntityIdRead) -> Option<T>
    {
        let entity_id = EntityId::new(entity_id_read.get_entity_instance(), ForceCreationEntity);
        self.get_copy::<T>(entity_id)
    }

    /// Get reference to a component from a read-only reference to an entity
    ///
    /// `entity_id_read` - the read key of the entity whose component should be referenced
    pub fn get_ref_read<'a, T: 'static + Copy + Serialize + Deserialize<'a>>(&self, entity_id_read: EntityIdRead) -> Option<&T>
    {
        let entity_id = EntityId::new(entity_id_read.get_entity_instance(), ForceCreationEntity);
        self.get_ref::<T>(entity_id)
    }

    /// Get a copy of a component
    ///
    /// `entity_id` - the Id of the entity associated with the desired component
    ///
    /// ```
    /// struct Position(u32);
    /// let entity_id = 3;
    /// let position_instance = ecs.get_copy::<Position>(entity_id).unwrap();
    /// ```
    pub fn get_copy<'a, T: 'static + Copy + Serialize + Deserialize<'a>>(&self, entity_id: EntityId) -> Option<T>
    {
        if let Some(data_reference) = self.get_ref::<T>(entity_id)
        {
            return Some(data_reference.clone());
        }

        None
    }

    /// Returns the type of object the given entity id
    ///
    /// `entity_id` - the ID of the entity that has its type being queried
    pub fn get_entity_type(&self, entity_id: EntityId) -> Option<TypeIdentifier>
    {
        self.get_copy::<TypeIdentifier>(entity_id)
    }

    pub fn get_entity_type_read(&self, entity_id: EntityIdRead) -> Option<TypeIdentifier>
    {
        let entity_id = EntityId::new(entity_id.get_entity_instance(), ForceCreationEntity);
        self.get_copy::<TypeIdentifier>(entity_id)
    }

    pub fn get_entity_model_type(&self, entity_id: EntityId) -> Option<ModelId>
    {
        self.get_copy::<ModelId>(entity_id)
    }

    /// Get a reference of a component
    ///
    /// `object_id` - the Id of the entity associated with the desired component
    ///
    /// ```
    /// struct Position(u32);
    /// let entity_id = 3;
    /// let position_instance = ecs.get_ref::<Position>(entity_id).unwrap();
    /// ```
    pub fn get_ref<'a, T: 'static + Serialize + Deserialize<'a>>(&self, entity_id: EntityId) -> Option<&T>
    {
        if let Some(index) = self.index_of::<T>()
        {
            match self.check_component_written::<T>(entity_id)
            {
                Ok(true) =>
                    {
                        return Some(self.registered_types[index].get_ref(entity_id));
                    },
                _ => return None
            }
        }

        None
    }

    /// Get a reference of a component
    ///
    /// `entity_id` - the Id of the entity associated with the desired component
    ///
    /// ```
    /// struct Position(u32);
    /// let entity_id = 3;
    /// let position_instance = ecs.get_ref_mut::<Position>(entity_id).unwrap();
    /// ```
    pub fn get_ref_mut<'a, T: 'static + Serialize + Deserialize<'a>>(&mut self, entity_id: EntityId) -> Option<&mut T>
    {
        if let Some(index) = self.index_of::<T>()
        {
            match self.check_component_written::<T>(entity_id)
            {
                Ok(true) =>
                    {
                        return Some(self.registered_types[index].get_ref_mut(entity_id));
                    },
                _ => return None
            }
        }

        None
    }

    /// Gets the index of a component type
    ///
    /// ```
    /// let index = self.index_of::<T>().unwrap();
    /// ```
    fn index_of<T: 'static>(&self) -> Option<usize>
    {
        let type_id = TypeIdentifier::from(TypeId::of::<T>());

        self.registered_types.iter().position(|x| x.type_id == type_id)
    }
}

impl IndexInformation
{
    /// Creates a new IndexInformation object
    ///
    /// `type_id` - the identifier of the type that this IndexInformation instance is representing
    ///
    /// ```
    /// let indexInformation = IndexInformation::new();
    /// ```
    fn new(type_id: TypeIdentifier) -> IndexInformation
    {
        IndexInformation{ type_id, instances: Vec::new(), free_space: Vec::new(), sparse_map: HashMap::default() }
    }

    /// Get the index of the component in the appropriate vector [holding the component]
    ///
    /// If the entity does not have the component, None is returned.
    ///
    /// `entity_id` - the Id of the entity for which to get the index of a component
    ///
    /// ```
    /// struct Position;
    /// let index = indexInformation.get_index::<Position>(anEntity);
    /// ```
    fn get_index<T: 'static>(&self, entity_id: EntityId) -> Option<usize>
    {
        match self.sparse_map.get(&entity_id)
        {
            Some(i) => Some((*i / std::mem::size_of::<T>() as isize) as usize),
            None => None
        }
    }

    /// Get a reference to component data
    ///
    /// `entity_id` - the Id of the entity associated with the desired component
    ///
    /// ```
    /// struct Position;
    /// let data = indexInformation.get_ref::<Position>(anEntity);
    /// ```
    fn get_ref<T: 'static>(&self, entity_id: EntityId) -> &T
    {
        if let Some(instance_index) = self.sparse_map.get(&entity_id)
        {
            unsafe
                {
                    return &(*(self.instances.as_ptr().offset(*instance_index) as *const T));
                }
        }
        else
        {
            panic!("Object ID {:?} is not stored for type: {:?}", entity_id,  TypeId::of::<T>());
        }
    }

    /// Get a mutable reference to component data
    ///
    /// `entity_id` - the Id of the entity associated with the desired component
    ///
    /// ```
    /// struct Position;
    /// let data = indexInformation.get_ref_mut::<Position>(anEntity);
    /// ```
    fn get_ref_mut<T: 'static>(&mut self, entity_id: EntityId) -> &mut T
    {
        if let Some(instance_index) = self.sparse_map.get(&entity_id)
        {
            unsafe
                {
                    return &mut (*(self.instances.as_mut_ptr().offset(*instance_index) as *mut T));
                }
        }
        else
        {
            panic!("Object ID {:?} is not stored for type: {:?}", entity_id,  TypeId::of::<T>());
        }
    }

    /// Remove the component data associated with the entity
    ///
    /// `entity_id` - the Id of the entity associated with the desired component
    ///
    /// ```
    ///  let entity_id = 0;
    ///  indexInformation.remove_data(entity_id);
    /// ```
    fn remove_data(&mut self, entity_id: EntityId)
    {
        if let Some(instance_index) = self.sparse_map.get(&entity_id)
        {
            self.free_space.push(*instance_index);
        }

        self.sparse_map.remove(&entity_id);
    }

    /// Write the value of a component for the given entity using the value's serialized form
    ///
    /// `entity_id` - the ID of the entity having its component updated
    /// `value` - the updated value of the component in its serialized form
    fn write_serialized_data(&mut self, entity_id: EntityId, value: &Vec<u8>)
    {
        // Space for the component already allocated; overwrite previous value
        if let Some(instance_index) = self.sparse_map.get(&entity_id)
        {
            unsafe
                {
                    copy_nonoverlapping(value.as_ptr(), self.instances.as_mut_ptr().offset(*instance_index), value.len());
                }
        }
        else
        {
            match self.free_space.pop()
            {
                // Reserved space for a component not currently being used; use that space
                Some(index) =>
                    {
                        unsafe
                            {
                                copy_nonoverlapping(value.as_ptr(), self.instances.as_mut_ptr().offset(index), value.len());
                            }

                        self.sparse_map.insert(entity_id, index);
                    },
                // Create new space for the components value
                None =>
                    {
                        let write_index = self.instances.len() as isize;

                        self.sparse_map.insert(entity_id, write_index);

                        // Remember that the objects are serialized into bytes; therefore the number
                        // of space the objects takes in bytes have to be allocated
                        for _ in 0..value.len()
                        {
                            self.instances.push(0);
                        }

                        unsafe
                            {
                                copy_nonoverlapping(value.as_ptr(), self.instances.as_mut_ptr().offset(write_index), value.len());
                            }
                    }
            }
        }
    }

    /// Writes component data for the given entity
    ///
    /// `entity_id` - the Id of the entity associated with the desired component
    /// `value` - value of the component to be written
    ///
    /// ```
    ///  let data: u32 = 0;
    ///  let entity_id = 0;
    ///  indexInformation.write_data::<u32>(entity_id, data);
    /// ```
    fn write_data<T>(&mut self, entity_id: EntityId, value: T)
    {
        // Space for the component already allocated; overwrite previous value
        if let Some(instance_index) = self.sparse_map.get(&entity_id)
        {
            unsafe
                {
                    *(self.instances.as_ptr().offset(*instance_index) as *mut T) = value;
                }
        }
        else
        {
            match self.free_space.pop()
            {
                // Reserved space for a component not currently being used; use that space
                Some(index) =>
                    {
                        unsafe
                            {
                                *(self.instances.as_ptr().offset(index) as *mut T) = value;
                            }

                        self.sparse_map.insert(entity_id, index);
                    },
                // Create new space for the components value
                None =>
                    {
                        let write_index = self.instances.len() as isize;

                        self.sparse_map.insert(entity_id, write_index);

                        // Remember that the objects are serialized into bytes; therefore the number
                        // of space the objects takes in bytes have to be allocated
                        for _ in 0.. std::mem::size_of::<T>()
                        {
                            self.instances.push(0);
                        }

                        unsafe
                            {
                                *(self.instances.as_ptr().offset(write_index) as *mut T) = value;
                            }
                    }
            }
        }
    }
}

#[cfg(test)]
mod tests
{
    use super::ECS;
    use super::{EntityId, IndexInformation};
    use super::super::entity_enforcers::ForceCreationEntity;
    use std::any::TypeId;
    use std::fmt::Debug;
    use std::collections::BTreeSet;
    use crate::objects::ecs::TypeIdentifier;
    use serde::{Serialize, Deserialize};

    #[derive(PartialEq, Eq, Debug, Copy, Clone, Serialize, Deserialize)]
    struct Position(u32);

    #[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
    struct Velocity(u32);

    #[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
    struct Acceleration(u32);

    #[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
    struct Marker;

    #[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
    struct Marker2;

    fn check_getters_some<'a, T>(ecs: &mut ECS, entity_id: EntityId, mut value: T)
        where T:
        Copy + Clone + Debug + PartialEq + Eq + 'static + Serialize + Deserialize<'a>
    {
        assert_eq!(Some(value), ecs.get_copy::<T>(entity_id));
        assert_eq!(Some(&value), ecs.get_ref::<T>(entity_id));
        assert_eq!(Some(&mut value), ecs.get_ref_mut::<T>(entity_id));
    }

    fn check_getters_none<'a, T>(ecs: &mut ECS, entity_id: EntityId)
        where T:
        Copy + Clone + Debug + PartialEq + Eq + 'static + Serialize + Deserialize<'a>
    {
        assert_eq!(None, ecs.get_copy::<T>(entity_id));
        assert_eq!(None, ecs.get_ref::<T>(entity_id));
        assert_eq!(None, ecs.get_ref_mut::<T>(entity_id));
    }

    fn cast_value<T>(index_information: &IndexInformation, index: isize) -> &T
    {
        unsafe
            {
                &*(index_information.instances.as_ptr().offset(index * std::mem::size_of::<T>() as isize) as *const T)
            }
    }

    #[test]
    fn register_write_component()
    {
        // >>> Checks all code paths in IndexInformation get_x functions and ECS get_x functions
        // >>> Checks code paths that affect state in ECS register_type function

        let mut ecs = ECS::new();
        ecs.register_type::<Position>();
        ecs.register_type::<Velocity>();

        let position_entity = ecs.create_entity();
        let velocity_entity = ecs.create_entity();

        ecs.write_component::<Position>(position_entity, Position(3));
        ecs.write_component::<Velocity>(velocity_entity, Velocity(5));

        check_getters_some::<Position>(&mut ecs, position_entity, Position(3));
        check_getters_none::<Velocity>(&mut ecs, position_entity);

        ecs.remove_entity(position_entity);

        check_getters_none::<Position>(&mut ecs, position_entity);
        check_getters_none::<Velocity>(&mut ecs, position_entity);
    }

    #[test]
    fn internal_representation_one_component()
    {
        // >>> Checks all code paths of IndexInformation's write_data() and remove_data()

        let mut ecs = ECS::new();
        ecs.register_type::<Position>();

        let first_entity = ecs.create_entity();
        let second_entity = ecs.create_entity();
        ecs.write_component::<Position>(first_entity, Position(3));
        ecs.write_component::<Position>(second_entity, Position(2));

        assert_eq!(&Position(3), cast_value::<Position>(&ecs.registered_types[1], 0));
        assert_eq!(&Position(2), cast_value::<Position>(&ecs.registered_types[1], 1));

        // Since the first entity was removed, the third entity should have its component written
        // in the removed entity's space
        let third_entity = ecs.create_entity();
        ecs.remove_entity(first_entity);

        assert_eq!(0, ecs.registered_types[1].free_space[0]);
        assert_eq!(1, ecs.registered_types[1].sparse_map.len()); // Still have the second entity

        ecs.write_component::<Position>(third_entity, Position(1));

        assert_eq!(&Position(1), cast_value::<Position>(&ecs.registered_types[1], 0));
        assert_eq!(&Position(2), cast_value::<Position>(&ecs.registered_types[1], 1));

        // Check that writing a component to the same entity twice overwrites the first time the component
        // was set for the entity
        ecs.write_component::<Position>(third_entity, Position(5));

        assert_eq!(&Position(5), cast_value::<Position>(&ecs.registered_types[1], 0));
        assert_eq!(&Position(2), cast_value::<Position>(&ecs.registered_types[1], 1));
    }

    #[test]
    fn check_component_written()
    {
        let mut ecs = ECS::new();

        ecs.register_type::<Position>();

        let entity = ecs.create_entity();

        ecs.register_type::<Velocity>();

        ecs.write_component::<Velocity>(entity, Velocity(0));

        assert_eq!(Ok(true), ecs.check_component_written::<Velocity>(entity));
        assert_eq!(Ok(false), ecs.check_component_written::<Position>(entity));
        assert_eq!(Err(()), ecs.check_component_written::<Acceleration>(entity));
    }

    #[test]
    fn check_removing_components()
    {
        let mut ecs = ECS::new();

        ecs.register_type::<Position>();
        ecs.register_type::<Velocity>();
        ecs.register_type::<Acceleration>();

        let entity = ecs.create_entity();

        ecs.write_component::<Position>(entity, Position(0));
        ecs.write_component::<Velocity>(entity, Velocity(1));
        ecs.write_component::<Acceleration>(entity, Acceleration(3));

        assert_eq!(14, ecs.bitsets[0][0]);
        check_getters_some::<Position>(&mut ecs, entity,Position(0));
        check_getters_some::<Velocity>(&mut ecs, entity,Velocity(1));
        check_getters_some::<Acceleration>(&mut ecs, entity,Acceleration(3));

        ecs.remove_component::<Velocity>(entity);

        assert_eq!(10, ecs.bitsets[0][0]);
        check_getters_some::<Position>(&mut ecs, entity,Position(0));
        check_getters_some::<Acceleration>(&mut ecs, entity,Acceleration(3));
        check_getters_none::<Velocity>(&mut ecs, entity);

        ecs.remove_component::<Acceleration>(entity);

        assert_eq!(2, ecs.bitsets[0][0]);
        check_getters_some::<Position>(&mut ecs, entity,Position(0));
        check_getters_none::<Acceleration>(&mut ecs, entity);
        check_getters_none::<Velocity>(&mut ecs, entity);

        ecs.remove_component::<Position>(entity);

        assert_eq!(0, ecs.bitsets[0][0]);
        check_getters_none::<Position>(&mut ecs, entity);
        check_getters_none::<Acceleration>(&mut ecs, entity);
        check_getters_none::<Velocity>(&mut ecs, entity);
    }

    #[test]
    fn register_write_several_components()
    {
        let mut ecs = ECS::new();

        ecs.register_type::<Position>();
        ecs.register_type::<Velocity>();

        let entity = ecs.create_entity();

        ecs.write_component::<Position>(entity, Position(1));
        ecs.write_component::<Velocity>(entity, Velocity(0));

        ecs.remove_entity(entity);

        ecs.register_type::<Acceleration>();

        let entity = ecs.create_entity();

        ecs.write_component::<Acceleration>(entity, Acceleration(0));

        check_getters_some::<Acceleration>(&mut ecs, entity, Acceleration(0));
        check_getters_none::<Position>(&mut ecs, entity);
        check_getters_none::<Velocity>(&mut ecs, entity);

        assert_eq!(8, ecs.bitsets[0][0]);

        assert_eq!(1, ecs.registered_types[1].free_space.len());
        assert!(ecs.registered_types[0].sparse_map.is_empty());

        assert_eq!(1, ecs.registered_types[2].free_space.len());
        assert!(ecs.registered_types[1].sparse_map.is_empty());
    }

    #[test]
    fn check_bitset_values()
    {
        let mut ecs = ECS::new();

        ecs.register_type::<Position>();
        ecs.register_type::<Velocity>();
        ecs.register_type::<Acceleration>();

        let entity = ecs.create_entity();

        assert_eq!(0, ecs.bitsets[0][0]);

        ecs.write_component::<Position>(entity, Position(0));
        assert_eq!(2, ecs.bitsets[0][0]);

        ecs.write_component::<Velocity>(entity, Velocity(0));
        assert_eq!(6, ecs.bitsets[0][0]);

        ecs.write_component::<Acceleration>(entity, Acceleration(0));
        assert_eq!(14, ecs.bitsets[0][0]);
    }

    #[test]
    #[should_panic]
    fn write_non_registered_component()
    {
        let mut ecs = ECS::new();

        let entity = ecs.create_entity();

        ecs.write_component::<Position>(entity, Position(1));
    }

    #[test]
    fn get_entities_with_components()
    {
        let mut ecs = ECS::new();

        ecs.register_type::<Position>();
        ecs.register_type::<Velocity>();

        let mut entities = Vec::new();

        for x in 0..10
        {
            entities.push(ecs.create_entity());
        }

        for x in (0..10usize).filter(|x| x % 2 == 0)
        {
            ecs.write_component::<Position>(entities[x], Position(2));
        }

        for x in (0..10usize).filter(|x| x % 3 == 0)
        {
            ecs.write_component::<Velocity>(entities[x], Velocity(2));
        }

        let pos_array = [TypeIdentifier::from(TypeId::of::<Position>())];
        let velocity_array = [TypeIdentifier::from(TypeId::of::<Velocity>())];
        let pos_velocity_array = [pos_array[0], velocity_array[0]];

        let requested_components_position = ecs.get_indexes_for_components(&pos_array);
        let requested_components_velocity = ecs.get_indexes_for_components(&velocity_array);
        let requested_components_both = ecs.get_indexes_for_components(&pos_velocity_array);

        let mut expected_entities_position = BTreeSet::new();
        expected_entities_position.insert(EntityId::new(0, ForceCreationEntity));
        expected_entities_position.insert(EntityId::new(2, ForceCreationEntity));
        expected_entities_position.insert(EntityId::new(4, ForceCreationEntity));
        expected_entities_position.insert(EntityId::new(6, ForceCreationEntity));
        expected_entities_position.insert(EntityId::new(8, ForceCreationEntity));

        let mut expected_entities_velocity = BTreeSet::new();
        expected_entities_velocity.insert(EntityId::new(0, ForceCreationEntity));
        expected_entities_velocity.insert(EntityId::new(3, ForceCreationEntity));
        expected_entities_velocity.insert(EntityId::new(6, ForceCreationEntity));
        expected_entities_velocity.insert(EntityId::new(9, ForceCreationEntity));

        let mut expected_entities_both = BTreeSet::new();
        expected_entities_both.insert(EntityId::new(0, ForceCreationEntity));
        expected_entities_both.insert(EntityId::new(6, ForceCreationEntity));

        assert_eq!(expected_entities_position, requested_components_position);
        assert_eq!(expected_entities_velocity, requested_components_velocity);
        assert_eq!(expected_entities_both, requested_components_both);
    }

    #[test]
    fn check_marker()
    {
        let mut ecs = ECS::new();

        let first_entity = ecs.create_entity();
        let second_entity = ecs.create_entity();

        ecs.write_entity_type(first_entity, TypeIdentifier::from(TypeId::of::<Marker>()));
        ecs.write_entity_type(second_entity, TypeIdentifier::from(TypeId::of::<Marker>()));

        assert!(ecs.entity_model_lookup.get(&TypeIdentifier::from(TypeId::of::<Marker>())).unwrap().contains(&first_entity));
        assert!(ecs.entity_model_lookup.get(&TypeIdentifier::from(TypeId::of::<Marker>())).unwrap().contains(&second_entity));
        assert!(ecs.entity_model_lookup.get(&TypeIdentifier::from(TypeId::of::<Marker2>())).is_none());

        assert_eq!(Some(TypeIdentifier::from(TypeId::of::<Marker>())), ecs.get_entity_type(first_entity));
        assert_eq!(Some(TypeIdentifier::from(TypeId::of::<Marker>())), ecs.get_entity_type(second_entity));

        ecs.remove_entity_type(first_entity);

        assert_eq!(None, ecs.get_entity_type(first_entity));
        assert_eq!(Some(TypeIdentifier::from(TypeId::of::<Marker>())), ecs.get_entity_type(second_entity));

        ecs.remove_entity_type(second_entity);

        assert_eq!(None, ecs.get_entity_type(first_entity));
        assert_eq!(None, ecs.get_entity_type(second_entity));

        assert!(ecs.entity_model_lookup.contains_key(&TypeIdentifier::from(TypeId::of::<Marker>())));
        assert!(!ecs.entity_model_lookup.contains_key(&TypeIdentifier::from(TypeId::of::<Marker2>())));

        assert!(ecs.entity_model_lookup.get(&TypeIdentifier::from(TypeId::of::<Marker>())).unwrap().is_empty());
        assert!(ecs.entity_model_lookup.get(&TypeIdentifier::from(TypeId::of::<Marker2>())).is_none());
    }

    #[test]
    fn smaller_quantity_component_first()
    {
        let mut ecs = ECS::new();

        ecs.register_type::<Position>();
        ecs.register_type::<Velocity>();

        let entity1 = ecs.create_entity();
        let entity2 = ecs.create_entity();
        let entity3 = ecs.create_entity();

        ecs.write_component::<Position>(entity1, Position(1));

        ecs.write_component::<Velocity>(entity1, Velocity(1));
        ecs.write_component::<Velocity>(entity2, Velocity(2));
        ecs.write_component::<Velocity>(entity3, Velocity(3));

        let mut expected_entities_velocity = BTreeSet::new();
        expected_entities_velocity.insert(entity1);
        expected_entities_velocity.insert(entity2);
        expected_entities_velocity.insert(entity3);

        let velocity_array = [TypeIdentifier::from(TypeId::of::<Velocity>())];
        assert_eq!(expected_entities_velocity, ecs.get_indexes_for_components(&velocity_array));
    }

    #[test]
    fn get_entities_with_maker()
    {
        let mut ecs = ECS::new();

        let marker_type = TypeIdentifier::from(TypeId::of::<Marker>());

        let entity1 = ecs.create_entity();
        ecs.write_entity_type(entity1, marker_type);

        let components_with_markers = ecs.get_entities_with_type(marker_type);
        assert_eq!(1, components_with_markers.len());
        assert_eq!(Some(&entity1), components_with_markers.get(&entity1));
    }

    #[test]
    fn remove_entity_twice()
    {
        let mut ecs = ECS::new();

        let entity = ecs.create_entity();
        ecs.remove_entity(entity);
        ecs.remove_entity(entity);
    }

    #[test]
    fn serialize_and_deserialize()
    {
        let mut ecs = ECS::new();

        ecs.register_type::<Position>();
        ecs.register_type::<Velocity>();

        let entity = ecs.create_entity();

        ecs.write_component::<Position>(entity, Position(1));
        ecs.write_component::<Velocity>(entity, Velocity(0));

        let serialized = bincode::serialize(&ecs).unwrap();

        let mut new_ecs: ECS = bincode::deserialize(&serialized).unwrap();

        assert_eq!(new_ecs.entity_model_lookup, ecs.entity_model_lookup);
        assert_eq!(new_ecs.bitsets, ecs.bitsets);
        assert_eq!(new_ecs.free_indexes, ecs.free_indexes);
        assert_eq!(new_ecs.registered_types.len(), ecs.registered_types.len());

        for x in 0..new_ecs.registered_types.len()
        {
            assert_eq!(new_ecs.registered_types[x].type_id, ecs.registered_types[x].type_id);
            assert_eq!(new_ecs.registered_types[x].free_space, ecs.registered_types[x].free_space);
            assert_eq!(new_ecs.registered_types[x].instances, ecs.registered_types[x].instances);
            assert_eq!(new_ecs.registered_types[x].sparse_map, ecs.registered_types[x].sparse_map);
        }
    }
}