use std::any::TypeId;
use std::mem::size_of;
use serde::{Serialize, Deserialize};
use crate::exports::entity_transformer::EntityTransformationBuilder;
use crate::objects::ecs::{ECS, TypeIdentifier};
use crate::objects::entity_id::{EntityId, OwnedEntity, ReferencedEntity, SelfEntity};

/// Represents one of the possible operations that can be done to modify an entity.
#[derive(Clone, Serialize, Deserialize)]
pub enum EntityChangeInformation
{
    AddEntity(String, TypeIdentifier, EntityTransformationBuilder, EntityChangeRequest),
    AddOwnedEntity(SelfEntity, OwnedEntity),
    AddReferencedEntity(SelfEntity, ReferencedEntity),

    AddSortableComponent(EntityId, TypeIdentifier),
    RemoveSortableComponent(EntityId),

    ModifyRequest(EntityChangeRequest),
    RemoveComponent((EntityId, TypeIdentifier)),
    RemoveOwnedEntity(SelfEntity, OwnedEntity),
    RemoveReferencedEntity(SelfEntity, ReferencedEntity),
    DeleteRequest(EntityId),

    MakeObjectStatic(EntityId),
    WakeUpRequest(EntityId),
}

/// Required information to modify the value of a component for an entity. Component is automatically
/// registered for the entity when writing the component to it.
#[derive(Clone, Serialize, Deserialize)]
pub struct EntityChangeRequest
{
    pub entity_id: EntityId,
    pub type_id: Vec<(TypeIdentifier, Vec<u8>)>,
}

impl EntityChangeRequest
{
    /// Creates a new empty change request specific to the given entity
    ///
    /// entity_id` - the entity for which the change request is intended for
    pub fn new(entity_id: EntityId) -> EntityChangeRequest
    {
        EntityChangeRequest{ entity_id, type_id: Vec::new(),  }
    }

    /// Writes the specified change to the entity
    ///
    /// `ecs` - structure storing state of all of the entities
    /// `change_index` - the index of the change requested to apply
    pub fn apply_changes(&self, ecs: &mut ECS, change_index: usize)
    {
        unsafe
            {
                let (type_id, serialized_value) = &self.type_id[change_index];
                ecs.write_component_serialized(self.entity_id, *type_id, serialized_value);
            }
    }

    /// Specifies a change of a component for the entity associated with this change request
    ///
    /// `new_value` - the value that the entity should have
    pub fn add_new_change<T: 'static + Copy>(&mut self, new_value: T)
    {
        let type_id = TypeId::of::<T>();
        let size_value = size_of::<T>();
        let mut serialized_value: Vec<u8> = Vec::with_capacity(size_value);
        for _ in 0..size_value
        {
            serialized_value.push(0);
        }
        unsafe
            {
                *(serialized_value.as_mut_ptr() as *mut T) = new_value;
            }

        self.type_id.push((TypeIdentifier::from(type_id), serialized_value));
    }

    /// Specifies a removal of a component for the entity associated with this change request
    pub fn remove_component<T: 'static + Copy>(&mut self)
    {
        let type_id = TypeId::of::<T>();
        self.type_id.push((TypeIdentifier::from(type_id), Vec::new()));
    }

    /// Get the number of changes that were specified for the entity associated with this change request
    pub fn number_changes(&self) -> usize
    {
        self.type_id.len()
    }
}