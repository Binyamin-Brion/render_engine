use std::any::TypeId;
use std::time::Instant;
use hashbrown::{HashMap, HashSet};
use crate::exports::camera_object::Camera;
use crate::exports::light_components::FindLightType;
use crate::exports::logic_components::{IsOutOfBounds, OutOfBoundsLogic};
use crate::exports::movement_components::{Position, Rotation, Scale, TransformationMatrix};
use crate::flows::render_flow::RenderFlow;
use crate::models::model_definitions::{ModelId, OriginalAABB};
use crate::models::model_storage::ModelBankOwner;
use crate::objects::ecs::{ECS, TypeIdentifier};
use crate::objects::entity_change_request::{EntityChangeInformation, EntityChangeRequest};
use crate::objects::entity_id::EntityId;
use crate::threads::public_common_structures::FrameChange;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;
use crate::world::bounding_volumes::aabb::StaticAABB;

/// Arguments required to apply requested changes to entities
pub struct ChangeArgs<'a>
{
    pub bounding_box_tree: &'a mut BoundingBoxTree,
    pub camera: &'a mut Camera,
    pub ecs: &'a mut ECS,
    pub model_bank_owner: Option<&'a mut ModelBankOwner>,
    pub out_of_bounds_logic: &'a HashMap<TypeIdentifier, OutOfBoundsLogic>,
    pub render_flow: &'a mut RenderFlow,
}

/// Applies requested changes to entities to both the ECS and associated bounding box tree
///
/// `args` - the variables required to apply changes requested for entities
pub fn apply_change(mut args: ChangeArgs, mut changes: Option<&mut Vec<FrameChange>>)
{
    let mut kinematics_changed_entities = HashSet::default();
    let mut only_translation_changed_entities = HashSet::default();
    let mut deleted_changed_entities = HashSet::default();

    if let Some(ref mut changes) = changes
    {
        for x in changes.iter_mut()
        {
            if let FrameChange::EntityChange(changes) = x
            {
                for entity_change in changes
                {
                    match entity_change
                    {
                        EntityChangeInformation::AddEntity(model_name, entity_type, physical_init_info, other_init_info) =>
                            {
                                let entity_id = args.ecs.create_entity();
                                physical_init_info.entity_id = entity_id;
                                other_init_info.entity_id = entity_id;

                                kinematics_changed_entities.remove(&entity_id);
                                only_translation_changed_entities.remove(&entity_id);
                                deleted_changed_entities.remove(&entity_id);

                                let model_id = match args.model_bank_owner
                                {
                                    Some(ref mut i) => match i.lookup_model(model_name)
                                    {
                                        Some(model_id) => Some(*model_id),
                                        None => None,
                                    },
                                    None =>
                                        {
                                            eprintln!("Failed to get the model bank owner");
                                            debug_assert!(false);
                                            None
                                        }
                                };

                                if let Some(model_id) = model_id
                                {
                                    match args.model_bank_owner
                                    {
                                        Some(ref mut model_owner) => match model_owner.get_model_info(model_id)
                                        {
                                            Some(i) =>
                                                {
                                                    physical_init_info.apply_choices(i.aabb.aabb, args.ecs, args.bounding_box_tree);

                                                    model_owner.register_instances(model_id, 1);
                                                    args.ecs.write_component::<ModelId>(entity_id, model_id);
                                                    args.ecs.write_entity_type(entity_id, *entity_type);

                                                    apply_entity_change_requests(args.ecs, other_init_info, &mut kinematics_changed_entities, &mut only_translation_changed_entities, &mut deleted_changed_entities);
                                                },
                                            None =>
                                                {
                                                    eprintln!("Failed to get the model information for model: {}", model_name);
                                                    debug_assert!(false);
                                                }
                                        },
                                        None =>
                                            {
                                                eprintln!("Failed to get the model bank owner");
                                                debug_assert!(false);
                                            }
                                    }
                                }
                                else
                                {
                                    eprintln!("Failed to get the model id for: {}", model_name);
                                    debug_assert!(false);
                                }
                            },
                        EntityChangeInformation::AddOwnedEntity(owner, other) =>
                            {
                                args.ecs.add_owned_entity(*owner, *other);
                            },
                        EntityChangeInformation::AddReferencedEntity(owner, other) =>
                            {
                                args.ecs.add_referenced_entity(*owner, *other);
                            },
                        EntityChangeInformation::MakeObjectStatic(ref entity_id) =>
                            {
                                args.bounding_box_tree.remove_entity(*entity_id);
                                let aabb = args.ecs.get_ref::<StaticAABB>(*entity_id).unwrap();
                                let light_type = find_entity_light_type(&args, entity_id);

                                if let Err(_) = args.bounding_box_tree.add_entity(*entity_id, aabb, should_add_if_out_bounds(&args, *entity_id), true, light_type)
                                {
                                    debug_assert!(false);
                                }
                            },
                        EntityChangeInformation::WakeUpRequest(ref entity_id) =>
                            {
                                args.bounding_box_tree.remove_entity(*entity_id);
                                let aabb = args.ecs.get_ref::<StaticAABB>(*entity_id).unwrap();
                                let light_type = find_entity_light_type(&args, entity_id);

                                if let Err(_) = args.bounding_box_tree.add_entity(*entity_id, aabb, should_add_if_out_bounds(&args, *entity_id), false, light_type)
                                {
                                    debug_assert!(false)
                                }
                            },
                        EntityChangeInformation::AddSortableComponent(ref entity_id, type_id) =>
                            {
                                args.ecs.write_sortable_component(*entity_id, *type_id);
                            },
                        EntityChangeInformation::RemoveSortableComponent(ref entity_id) =>
                            {
                                args.ecs.remove_sortable_component(*entity_id);
                            },
                        EntityChangeInformation::RemoveOwnedEntity(owner, other) =>
                            {
                                args.ecs.remove_owned_entity(*owner, *other);
                            },
                        EntityChangeInformation::RemoveReferencedEntity(owner, other) =>
                            {
                                args.ecs.remove_referenced_entity(*owner, *other);
                            },
                        EntityChangeInformation::ModifyRequest(ref change_request) =>
                            {
                                apply_entity_change_requests(args.ecs, change_request, &mut kinematics_changed_entities, &mut only_translation_changed_entities, &mut deleted_changed_entities);
                            },
                        EntityChangeInformation::RemoveComponent((ref entity_id, ref type_id)) =>
                            {
                                args.ecs.remove_component_type_id_internal(*entity_id, *type_id);
                            },
                        EntityChangeInformation::DeleteRequest(ref entity_id) =>
                            {
                                // If modify requests were made before this branch, then the program is still in
                                // a valid state, but redundant work was done. Those same changes after this branch
                                // would be invalid

                                let model_index = args.ecs.get_copy::<ModelId>(*entity_id).unwrap();

                                if let Some(ref mut model_bank) = args.model_bank_owner
                                {
                                    model_bank.remove_instance( model_index);
                                }

                                args.bounding_box_tree.remove_entity(*entity_id);
                                kinematics_changed_entities.remove(entity_id);
                                deleted_changed_entities.insert(*entity_id);
                                args.ecs.remove_entity(*entity_id);
                            }
                    }
                }
            }
        }
    }

    update_aabb_after_kinematic_change(kinematics_changed_entities, only_translation_changed_entities, &mut args);

    args.bounding_box_tree.end_of_changes(&args.ecs);
}

fn find_entity_light_type(args: &ChangeArgs, entity_id: &EntityId) -> Option<FindLightType>
{
    if args.ecs.get_entities_with_sortable()[2].contains(entity_id)
    {
        Some(FindLightType::Point)
    }
    else if args.ecs.get_entities_with_sortable()[3].contains(entity_id)
    {
        Some(FindLightType::Spot)
    }
    else if args.ecs.get_entities_with_sortable()[1].contains(entity_id)
    {
        Some(FindLightType::Directional)
    }
    else
    {
        None
    }
}

/// Updates the entities that have had their kinematics information changed by updating their AABB
/// based off of their new kinematic information
///
/// `entities_moved` - the entities that have moved as a result of a change to their position, rotation
///                     or scale components
/// `args` - the variables required to apply changes requested for entities
pub fn update_aabb_after_kinematic_change(entities_moved: HashSet<EntityId>, only_translation_changed_entities: HashSet<EntityId>, args: &mut ChangeArgs)
{
    let time = Instant::now();

    for entity_id in only_translation_changed_entities
    {
        let position = args.ecs.get_copy::<Position>(entity_id).unwrap();
        let mut new_aabb = args.ecs.get_ref::<OriginalAABB>(entity_id).unwrap().aabb.clone();
        new_aabb.translate(position.get_position());

        {
            let mut transformation_matrix = args.ecs.get_ref_mut::<TransformationMatrix>(entity_id).unwrap().get_matrix();
            let mut column = nalgebra_glm::column(&transformation_matrix, 3);
            column.x = position.get_position().x;
            column.y = position.get_position().y;
            column.z = position.get_position().z;
            transformation_matrix = nalgebra_glm::set_column(&transformation_matrix, 3, &column);
            args.ecs.write_component::<TransformationMatrix>(entity_id, TransformationMatrix::new(transformation_matrix));
        }

        args.ecs.write_component::<StaticAABB>(entity_id, new_aabb);

        update_entity_in_tree(args, entity_id, &new_aabb);
    }

    for entity_id in entities_moved
    {
        let position = args.ecs.get_ref::<Position>(entity_id).unwrap();
        let rotation = args.ecs.get_copy::<Rotation>(entity_id).unwrap_or_else(|| Rotation::default());
        let scale = args.ecs.get_copy::<Scale>(entity_id).unwrap_or_else(|| Scale::default());

        let mut transformation_matrix = nalgebra_glm::translate(&nalgebra_glm::identity(), &position.get_position());
        transformation_matrix = nalgebra_glm::rotate(&transformation_matrix, rotation.get_rotation(), &rotation.get_rotation_axis());
        transformation_matrix = nalgebra_glm::scale(&transformation_matrix, &scale.get_scale());

        let transformation_matrix = TransformationMatrix::new(transformation_matrix);

        let new_aabb = args.ecs.get_ref::<OriginalAABB>(entity_id).unwrap().aabb.clone().apply_transformation(&transformation_matrix.get_matrix());
        args.ecs.write_component::<StaticAABB>(entity_id, new_aabb);
        args.ecs.write_component::<TransformationMatrix>(entity_id, transformation_matrix);

        update_entity_in_tree(args, entity_id, &new_aabb);
    }

    println!("{}", time.elapsed().as_millis());
}

fn should_add_if_out_bounds(args: &ChangeArgs, entity_id: EntityId) -> bool
{
    if let Some(entity_type) = args.ecs.get_entity_type(entity_id)
    {
        args.out_of_bounds_logic.get(&entity_type).is_some()
    }
    else
    {
        false
    }
}

fn apply_entity_change_requests(ecs: &mut ECS, change_request: &EntityChangeRequest,
                                kinematics_changed_entities: &mut HashSet::<EntityId>,
                                only_translation_changed_entities: &mut HashSet<EntityId>,
                                deleted_changed_entities: &mut HashSet::<EntityId>)
{
    if deleted_changed_entities.contains(&change_request.entity_id)
    {
        return;
    }

    let mut position_changed = false;
    let mut rotation_changed = false;
    let mut scale_changed = false;

    for i in 0..change_request.number_changes()
    {
        // StaticAABB is only changed indirectly through a transformation. This is to ensure consistency.
        // If for example, entity moved right 10 units, but moved its AABB left 10 units, then the system
        // is now in an inconsistent state
        if change_request.type_id[i].0 == TypeIdentifier::from(TypeId::of::<StaticAABB>())
        {
            debug_assert!(false, "Attempted to modify StaticAABB directly for entity {:?}", change_request.entity_id);
            continue;
        }

        change_request.apply_changes(ecs, i);
        // Changes to these types will change the entity's AABB. However, only the
        // last change request received for each of these types will be applied to the
        // new AABB. These changes are delayed until the last change request is performed
        // to reduce calculations done when updating the bounding box tree
        position_changed |= change_request.type_id[i].0 == TypeIdentifier::from(TypeId::of::<Position>());
        rotation_changed |= change_request.type_id[i].0 == TypeIdentifier::from(TypeId::of::<Rotation>());
        scale_changed |= change_request.type_id[i].0 == TypeIdentifier::from(TypeId::of::<Scale>());
    }

    if position_changed && !rotation_changed && !scale_changed
    {
        if !kinematics_changed_entities.contains(&change_request.entity_id)
        {
            only_translation_changed_entities.insert(change_request.entity_id);
        }
    }
    else if position_changed || rotation_changed || scale_changed
    {
        kinematics_changed_entities.insert(change_request.entity_id);
        only_translation_changed_entities.remove(&change_request.entity_id);
    }
}

fn update_entity_in_tree(args: &mut ChangeArgs, entity_id: EntityId, aabb: &StaticAABB)
{
    // Every entity should have an entity type, but if it does not, have this check to prevent a crash
    let add_if_out_bounds = should_add_if_out_bounds(&args, entity_id);
    let light_type = find_entity_light_type(args, &entity_id);

    if args.bounding_box_tree.add_entity(entity_id,aabb, add_if_out_bounds, false, light_type).is_err()
    {
        if add_if_out_bounds
        {
            args.ecs.write_component::<IsOutOfBounds>(entity_id, IsOutOfBounds);
        }
        else
        {
            // If there is no logic to deal with out of bound entities in the next frame,
            // then there is no known way to deal with it, so it is deleted
            let model_index = args.ecs.get_copy::<ModelId>(entity_id).unwrap();

            if let Some(ref mut model_bank_owner) = args.model_bank_owner
            {
                model_bank_owner.remove_instance( model_index);
            }

            args.ecs.remove_entity(entity_id)
        }
    }
}