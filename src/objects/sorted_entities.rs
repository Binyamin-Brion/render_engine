use std::any::TypeId;
use std::iter::FromIterator;
use hashbrown::{HashMap, HashSet};
use serde::{Serialize, Deserialize};
use crate::objects::ecs::TypeIdentifier;
use crate::objects::entity_id::EntityId;

/// Sorts entities by components, allowing for groups of entities of the same model type to be queried
/// based off of a component that only some of those entities have
#[derive(Clone, Serialize, Deserialize)]
pub struct EntityComponentOrganizer
{
    components: Vec<SortableComponent>,
    reverse_lookup: HashMap<EntityId, usize>,
}

/// A component that allows sorting entities that an entity can have
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortableComponent
{
    type_id: TypeIdentifier,
    pub entities: HashSet::<EntityId>,
}

impl EntityComponentOrganizer
{
    /// Create a organizer that has the given components that entities can be sorted on
    ///
    /// `initial_components` - the components that entities can be sorted on
    pub fn new(mut sortable_components: Vec<TypeIdentifier>) -> EntityComponentOrganizer
    {
        // Default sortable component that all entities have is to ensure the indexes of light sortable
        // components remain constant
        sortable_components.insert(0, TypeIdentifier::from(TypeId::of::<TypeIdentifier>()));

        let components = sortable_components.iter()
            .map(|x| SortableComponent{ type_id: *x, entities: Default::default() }).collect::<Vec<SortableComponent>>();

        EntityComponentOrganizer{ components, reverse_lookup: HashMap::default() }
    }

    /// Add a sortable component to an entity, removing the old sortable component. If the sortable
    /// component has not been added to the organizer, then it is added
    ///
    /// `entity_id` - the ID of the entity to add a sortable component to
    /// `type_id` - the type ID of the sortable component the entity will have added to it
    pub fn add_entity(&mut self, entity_id: EntityId, type_id: TypeIdentifier)
    {
        self.remove_entity(entity_id);

        let index = match self.components.iter().position(|x| x.type_id == type_id)
        {
            Some(i) => i,
            None =>
                {
                    self.components.push(SortableComponent{ type_id, entities: HashSet::default() });
                    self.components.len() - 1

                }
        };

        self.components[index].entities.insert(entity_id);
        self.reverse_lookup.insert(entity_id, index);
    }

    /// Add the default sortable component to an entity, removing the old sortable component
    ///
    /// `entity_id` - the ID of the entity to add the default sortable component to
    pub fn add_entity_default_component(&mut self, entity_id: EntityId)
    {
        self.add_entity(entity_id, TypeIdentifier::from(TypeId::of::<TypeIdentifier>()));
    }

    /// Remove an entity from any sortable component associated with it
    ///
    /// `entity_id` - the ID of the entity to be removed
    fn remove_entity(&mut self, entity_id: EntityId)
    {
        if let Some(type_index) = self.reverse_lookup.get(&entity_id)
        {
            self.components[*type_index].entities.remove(&entity_id);
            self.reverse_lookup.remove(&entity_id);
        }
    }

    /// Get the number of sortable components stored in the organizer
    pub fn number_sortable_components(&self) -> usize
    {
        self.components.len()
    }

    /// Get all of the entities that are associated with a sortable component
    pub fn get_entities_with_sortable_components(&self) -> Vec<&HashSet::<EntityId>>
    {
        Vec::from_iter(self.components.iter().map(|x| &x.entities))
    }
}