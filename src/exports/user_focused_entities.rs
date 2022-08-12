use std::any::TypeId;
use crate::objects::ecs::TypeIdentifier;

pub struct UserEntity;

pub fn user_type_identifier() -> TypeIdentifier
{
    TypeIdentifier::from(TypeId::of::<UserEntity>())
}