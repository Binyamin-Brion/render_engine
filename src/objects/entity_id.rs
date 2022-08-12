use serde::{Serialize, Deserialize};
use crate::objects::entity_enforcers::ForceCreationEntity;

/// Represents a created entity in the ECS.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityId(u32);

/// Represents a read-only reference to an entity created in the ECS.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityIdRead(u32);

pub type SelfEntity = EntityId;
pub type OwnedEntity = EntityId;
pub type ReferencedEntity = EntityIdRead;

impl EntityId
{
    /// Constructs a new EntityId representing the given entity instance
    ///
    /// `entity_instance` - the global instance of an entity this entity will have
    /// `ForceCreationEntity` - structure that restricts where creation of an entity can occur
    pub fn new(entity_instance: u32, _: ForceCreationEntity) -> EntityId
    {
        EntityId(entity_instance)
    }

    /// Get the entity instance of this entity
    pub fn get_entity_instance(&self) -> u32
    {
        // This function exists instead of public member variable to prevent user from accidentally
        // changing what entity this object refers to
        self.0
    }
}

impl EntityIdRead
{
    /// Constructs a new EntityReadId representing the given entity instance
    ///
    /// `entity_id` - the id of the entity that should have a read key created for it
    pub fn new(entity_id: EntityId) -> EntityIdRead
    {
        EntityIdRead(entity_id.0)
    }

    /// Get the entity instance of this entity.
    pub fn get_entity_instance(&self) -> u32
    {
        self.0
    }
}