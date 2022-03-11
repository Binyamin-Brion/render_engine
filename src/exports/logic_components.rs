use serde::{Serialize, Deserialize};
use crate::objects::ecs::ECS;
use crate::objects::entity_id::{EntityId, EntityIdRead};
use crate::objects::entity_change_request::EntityChangeInformation;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;

type SelfEntity = EntityId;
type OtherEntity = EntityIdRead;
type CurrentFrameECS = ECS;
type ElapsedTime = f32;

type LogicFunction = fn(SelfEntity, &CurrentFrameECS, &BoundingBoxTree, ElapsedTime) -> Option<EntityChangeInformation>;
type CollisionFunction = fn(SelfEntity, OtherEntity, &CurrentFrameECS, &BoundingBoxTree) -> Option<EntityChangeInformation>;
type OutOfBoundsFunction = fn(SelfEntity, &mut CurrentFrameECS);

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenderSystemIndex
{
    pub index: usize
}

/// Note: if the LogicFunction will issue a DeleteRequest, then LogicFunction must return an EntityChangeInformation
/// with ONLY that Delete request.
//#[derive(Copy, Clone)]
pub struct EntityLogic
{
    pub logic: LogicFunction,
}

#[derive(Copy, Clone)]
pub struct CollisionLogic
{
    pub logic: CollisionFunction,
}

#[derive(Copy, Clone)]
pub struct OutOfBoundsLogic
{
    pub logic: OutOfBoundsFunction
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct IsOutOfBounds;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct ParentEntity{ pub entity: EntityId }

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct CanCauseCollisions;