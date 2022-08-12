/// Forces the creation of an EntityId to be done only from an ECS; prevents user from creating
/// a "key" to an entity that was never created.
pub struct ForceCreationEntity;