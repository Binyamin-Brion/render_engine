use std::any::TypeId;
use std::iter::FromIterator;
use std::mem::swap;
use std::sync::Arc;
use std::time::Instant;
use float_cmp::approx_eq;
use hashbrown::{HashMap, HashSet};
use lazy_static::lazy_static;
use nalgebra_glm::{TVec3, vec3};
use parking_lot::{Mutex, RwLock};
use rayon::iter::ParallelIterator;
use rayon::prelude::ParallelSliceMut;
use crate::culling::logic_frustum_culler::LogicFrustumCuller;
use crate::culling::render_frustum_culler::RenderFrustumCuller;
use crate::culling::r#trait::TraversalDecider;
use crate::exports::camera_object::{Camera, MovementFactor};
use crate::exports::light_components::LightInformation;
use crate::exports::load_models::{InstanceLogic, RegisterInstancesFunction};
use crate::exports::logic_components::{UserAlwaysCausesCollisions, CanCauseCollisions, IsOutOfBounds, ParentEntity, RenderSystemIndex, UserInputLogic, AlwaysExecuteLogic};
use crate::exports::movement_components::{Acceleration, AccelerationRotation, HasMoved, HasRotated, Position, Rotation, Scale, TransformationMatrix, Velocity, VelocityRotation};
use crate::flows::render_flow::RenderFlow;
use crate::flows::visible_world_flow::CullResult;
use crate::helper_things::aabb_helper_functions;
use crate::helper_things::aabb_helper_functions::distance_to_aabb;
use crate::helper_things::cpu_usage_reducer::TimeTakeHistory;
use crate::helper_things::entity_change_helpers::{apply_change, ChangeArgs};
use crate::models::model_definitions::{ModelId, OriginalAABB};
use crate::models::model_storage::ModelBankOwner;
use crate::objects::ecs::{ECS, TypeIdentifier};
use crate::objects::entity_change_request::{EntityChangeInformation, EntityChangeRequest};
use crate::objects::entity_id::{EntityId, EntityIdRead};
use crate::threads::public_common_structures::FrameChange;
use crate::window::input_state::{CurrentFrameInput, InputHistory};
use crate::world::bounding_box_tree_v2::{BoundingBoxTree, SharedWorldSectionId, UniqueWorldSectionId, WorldSectionLookup};
use crate::world::bounding_volumes::aabb::StaticAABB;

lazy_static!
{
    static ref POSITION_TIME_HISTORY: Mutex<TimeTakeHistory> = Mutex::new(TimeTakeHistory::new());
    static ref COLLISION_TIME_HISTORY: Mutex<TimeTakeHistory> = Mutex::new(TimeTakeHistory::new());
    static ref LOGIC_TIME_HISTORY: Mutex<TimeTakeHistory> = Mutex::new(TimeTakeHistory::new());
}

/// Represents the logic of the entities within the game, making sure their logic is executed
/// as the game progresses.
pub struct LogicFlow
{
    pub ecs: ECS,
    last_accessed_time: Instant, // Keeps movement in units / second
moved_entities: Mutex<Vec<EntityId>>,
    expected_frame_changes: parking_lot::Mutex<Vec<FrameChange>>,
    random_frame_changes: parking_lot::Mutex<Vec<FrameChange>>,
    previous_camera_pos: TVec3<f32>,
    always_execute_entities: HashSet<EntityId>,

    pub instance_logic: InstanceLogic,
}

/// Holds the variables needed to compute one game loop logic for entities
pub struct ExecutionArgs<'a>
{
    pub visible_world_sections: CullResult,
    pub bounding_box_tree: &'a mut BoundingBoxTree,
    pub model_bank_owner: Arc<RwLock<ModelBankOwner>>,
    pub delta_time: f32,
    pub camera: &'a mut Camera,
    pub logic_frustum_culler: &'a LogicFrustumCuller,
    pub render_frustum_culler: &'a RenderFrustumCuller,
    pub input_history: &'a InputHistory,
    pub current_input: &'a CurrentFrameInput,
}

impl LogicFlow
{
    /// Creates a new LogicFlow, with an empty ECS.
    ///
    /// `instance_logic` - the variable holding the logic for different scenarios for each type of entity
    pub fn new(instance_logic: InstanceLogic, register_instances: Vec<RegisterInstancesFunction>) -> LogicFlow
    {
        let mut ecs = ECS::new();

        ecs.register_type::<UserAlwaysCausesCollisions>();
        ecs.register_type::<CanCauseCollisions>();
        ecs.register_type::<HasMoved>();
        ecs.register_type::<Position>();
        ecs.register_type::<Velocity>();
        ecs.register_type::<Acceleration>();

        ecs.register_type::<HasRotated>();
        ecs.register_type::<Rotation>();
        ecs.register_type::<VelocityRotation>();
        ecs.register_type::<AccelerationRotation>();

        ecs.register_type::<Scale>();
        ecs.register_type::<TransformationMatrix>();

        ecs.register_type::<ModelId>();
        ecs.register_type::<RenderSystemIndex>();

        ecs.register_type::<StaticAABB>();
        ecs.register_type::<OriginalAABB>();

        ecs.register_type::<IsOutOfBounds>();
        ecs.register_type::<ParentEntity>();

        ecs.register_type::<LightInformation>();

        ecs.register_type::<AlwaysExecuteLogic>();

        ecs.register_type::<MovementFactor>();

        for x in register_instances
        {
            x(&mut ecs);
        }

        let logic_flow = LogicFlow
        {
            /*
                The number_entity_change_requests is used because the change_request_receiver needs to
                loop only over the sent message in this frame; it should not wait for message from later
                frames. This is because it runs in this thread- it would block the thread that starts the
                next frame, stalling the program.

                Sending a Done signal to the receiver could lead to issues, as the order of messages
                received is not known. It should be FIFO, but if reasoning for this is incorrect, then
                change requests may not be processed as the receiver gets the Done message first.

                Thus the number of change requests is tracked, and the receiver keeps polling for more
                messages until it has processed the required number of change requests.
             */

            ecs,
            last_accessed_time: Instant::now(),
            moved_entities: Mutex::new(Vec::new()),
            expected_frame_changes: parking_lot::Mutex::new(Vec::new()),
            random_frame_changes: parking_lot::Mutex::new(Vec::new()),
            previous_camera_pos: vec3(0.0, 0.0, 0.0),
            instance_logic,
            always_execute_entities: HashSet::new()
        };


        logic_flow
    }

    /// Creates a new logic flow from entity states that were loaded elsewhere in the program
    ///
    /// `ecs` - the state of entities to initialize the logic flow with
    pub fn new_from_loaded_state(ecs: ECS, instance_logic: InstanceLogic) -> LogicFlow
    {
        LogicFlow
        {
            ecs,
            last_accessed_time: Instant::now(),
            moved_entities: Mutex::new(Vec::new()),
            expected_frame_changes: parking_lot::Mutex::new(Vec::new()),
            random_frame_changes: parking_lot::Mutex::new(Vec::new()),
            previous_camera_pos: vec3(0.0, 0.0, 0.0),
            instance_logic,
            always_execute_entities: HashSet::new()
        }
    }

    pub fn execute_user_input(&mut self, args: ExecutionArgs, input_functions: &Vec<UserInputLogic>)
    {
        let user_id = self.ecs.get_user_id();

        let mut entity_changes = Vec::new();
        for x in input_functions
        {
            let changes = (x.logic)(user_id, &self.ecs, args.bounding_box_tree, args.camera, args.input_history, args.current_input, args.delta_time);
            entity_changes.push(FrameChange::EntityChange(changes));
        }
        *self.expected_frame_changes.lock() = entity_changes;
    }

    /// Executes the logic of a single game loop
    ///
    /// `args` - the required variables to execute a game frame logic
    pub fn execute_logic(&mut self, args: ExecutionArgs, render_flow: &mut RenderFlow) -> Vec<FrameChange>
    {
        self.last_accessed_time = Instant::now();

        {
            let mut change_history = self.random_frame_changes.lock();

            change_history.push(FrameChange::DeltaTime(args.delta_time));

            if args.camera.get_view_matrix_changed()
            {
                change_history.push(FrameChange::CameraViewChange(args.camera.get_serializable_data()));
            }
            else
            {
                change_history.push(FrameChange::CameraStationary);
            }

            if args.camera.get_draw_param_changed()
            {
                change_history.push(FrameChange::DrawDistancesChange
                    (
                        args.camera.get_near_draw_distance(),
                        args.camera.get_far_draw_distance(),
                        args.camera.get_fov()
                    ));
            }

            if args.camera.get_window_dimensions_changed()
            {
                self.random_frame_changes.lock().push(FrameChange::WindowDimensionsChange(args.camera.get_window_dimensions()))
            }
        }

        // Only need to process world sections that have entities that are active
        let mut active_world_sections = Vec::new();
        for x in &args.visible_world_sections.visible_sections_vec
        {
            if args.bounding_box_tree.is_section_active(*x)
            {
                active_world_sections.push(*x);
            }
        }

        self.find_always_execute_entities(args.bounding_box_tree, &args.visible_world_sections);

        let user_id = self.ecs.get_user_id();
        self.ecs.write_component::<Position>(user_id, Position::new(args.camera.get_position()));
        self.handle_out_of_bounds_entities(args.bounding_box_tree, args.model_bank_owner.clone());
        self.update_positions(&active_world_sections, &args);

        let same_position =   approx_eq!(f32, self.previous_camera_pos.x, args.camera.get_position().x, ulps = 2) &&
            approx_eq!(f32, self.previous_camera_pos.y, args.camera.get_position().y, ulps = 2) &&
            approx_eq!(f32, self.previous_camera_pos.z, args.camera.get_position().z, ulps = 2);

        if  self.ecs.check_component_written_assume_registered::<UserAlwaysCausesCollisions>(user_id) ||
            (!same_position && self.ecs.check_component_written_assume_registered::<CanCauseCollisions>(user_id))
        {
            self.moved_entities.lock().push(user_id);
        }

        self.previous_camera_pos = args.camera.get_position();

        self.handle_collisions(&args);
        self.update_logic(&active_world_sections, &args);

        // Add the updated user entity AABB to the bounding box tree
        args.bounding_box_tree.remove_entity(user_id);
        let mut actual_user_aabb = self.ecs.get_copy::<OriginalAABB>(user_id).unwrap().aabb;
        actual_user_aabb.translate(args.camera.get_position());
        self.ecs.write_component::<StaticAABB>(user_id, actual_user_aabb);
        args.bounding_box_tree.add_entity(user_id, &actual_user_aabb, false, false, None).unwrap();
        args.bounding_box_tree.end_of_changes(&mut self.ecs);

        self.update_bounding_box_tree(&mut *args.bounding_box_tree, args.model_bank_owner, args.camera, render_flow);

        let new_position = self.ecs.get_copy::<Position>(user_id).unwrap().get_position();
        args.camera.force_hard_position(new_position);

        self.expected_frame_changes.lock().clear();

        let mut new_random_frame_changes = Vec::with_capacity(self.random_frame_changes.lock().len());
        swap(&mut new_random_frame_changes, &mut *self.random_frame_changes.lock());
        new_random_frame_changes
    }

    /// Applies out of bounds logic to entities that have moved past the valid positions of the world
    ///
    /// `bounding_box_tree` - the tree holding all of the entities
    /// `model_bank_owner` - owner of all of the geometric models that represent the entities
    pub fn handle_out_of_bounds_entities(&mut self, bounding_box_tree: &mut BoundingBoxTree, model_bank_owner: Arc<RwLock<ModelBankOwner>>)
    {
        let type_id = [TypeIdentifier::from(TypeId::of::<IsOutOfBounds>())];
        let out_of_bounds_entities = self.ecs.get_indexes_for_components(&type_id);

        for entity in out_of_bounds_entities
        {
            // This check is not strictly required; if an entity does not have a out of bounds logic function
            // then it will be deleted (see entity_change_helpers file) at the end of the logic flow.
            // This check is still included as a failsafe, in case something went wrong
            if let Some(entity_type) = self.ecs.get_entity_type(entity)
            {
                if let Some(bounds_logic) = self.instance_logic.out_of_bounds_logic.get(&entity_type)
                {
                    (bounds_logic.logic)(entity, &mut self.ecs);

                    let entity_aabb = self.ecs.get_ref::<StaticAABB>(entity).unwrap();
                    if aabb_helper_functions::aabb_out_of_bounds(entity_aabb, bounding_box_tree.outline_length() as f32)
                    {
                        let model_index = self.ecs.get_copy::<ModelId>(entity).unwrap();
                        model_bank_owner.write().remove_instance( model_index);

                        bounding_box_tree.remove_entity(entity);
                        self.ecs.remove_entity(entity);
                    }

                    self.ecs.remove_component::<IsOutOfBounds>(entity);
                }
            }
        }
    }

    /// Updates the positions of the entities that have kinematics applied. The given bounding box tree is
    /// updated to account for the change in position of entities.
    ///
    /// `affected_world_ids` - list of world sections, and only entities in these world sections have their position updated
    /// `args` - variables required to perform the entity kinematic updates
    pub fn update_positions(&mut self, affected_world_ids: &Vec<UniqueWorldSectionId>, args: &ExecutionArgs)
    {
        self.reset_has_changed_component();

        // Reserve required capacity for entities that will move; quick operation to perform that could reduce
        // many redundant re-allocations
        let type_id = [TypeIdentifier::from(TypeId::of::<Acceleration>())];
        let entities_with_velocity = self.ecs.get_indexes_for_components(&type_id);
        self.moved_entities = Mutex::new(Vec::with_capacity(entities_with_velocity.len()));

        let processed_world_sections: Mutex<HashSet<SharedWorldSectionId>> = Mutex::new(HashSet::default());

        let updated_kinematics_fn = |chunk_world_sections: &[UniqueWorldSectionId]|
            {
                for world_section in chunk_world_sections
                {
                    if let Some(entities_in_section) = args.bounding_box_tree.stored_entities_indexes.get(&world_section)
                    {
                        LogicFlow::apply_kinematics(&self, &entities_in_section.local_entities, args.delta_time);

                        for shared_world_section_index in &entities_in_section.shared_sections_ids
                        {
                            // True if the value was not in map when inserting
                            if processed_world_sections.lock().insert(*shared_world_section_index)
                            {
                                match args.bounding_box_tree.shared_section_indexes.get(shared_world_section_index)
                                {
                                    Some(i) =>
                                        {
                                            // Shared section can extend past unique world section, away from the camera.
                                            // Even if the aforementioned unique section is visible, shared section might not be
                                            if args.logic_frustum_culler.aabb_in_view(&i.aabb) ||
                                                args.render_frustum_culler.aabb_in_view(&i.aabb)
                                            {
                                                LogicFlow::apply_kinematics(&self, &i.entities, args.delta_time);
                                            }
                                        },
                                    // This is a property of the bounding tree- a world section only points to
                                    // a shared section when that shared section exists- if all entities in that
                                    // shared section are removed, the world section no longer points to it
                                    None => unreachable!()
                                }
                            }
                        }
                    }
                }
            };

        TimeTakeHistory::apply_to_function(&mut *POSITION_TIME_HISTORY.lock(), updated_kinematics_fn, affected_world_ids);
        LogicFlow::apply_kinematics(&self, &self.always_execute_entities, args.delta_time);
    }

    /// Takes in the set of entities and updates their kinematic information. Helper function to the
    /// update_positions function
    ///
    /// `logic_flow` - instance of the flow that manages the logic of entities
    /// `entities` - the entities that will have their kinematic information updated
    /// `elapsed_time` - the amount of time that has passed since the last game loop in seconds
    fn apply_kinematics(logic_flow: &LogicFlow, entities: &HashSet::<EntityId>, elapsed_time: f32)
    {
        for entity in entities
        {
            let mut entity_moved = false;

            // If an Entity has an acceleration component, then it has a velocity and position component.
            // The acceleration and velocity can be 0, but if one of components exist, then the other
            // one (including position) must exist as well
            if logic_flow.ecs.check_component_written_assume_registered::<Velocity>(*entity)
            {
                let mut entity_change_request = EntityChangeRequest::new(*entity);

                // Update the velocity based off of the acceleration
                if logic_flow.ecs.check_component_written_assume_registered::<Acceleration>(*entity)
                {
                    let acceleration = logic_flow.ecs.get_copy::<Acceleration>(*entity).unwrap();
                    let mut velocity = logic_flow.ecs.get_copy::<Velocity>(*entity).unwrap();
                    if nalgebra_glm::length(&acceleration.get_acceleration()) != 0.0
                    {
                        velocity += acceleration * elapsed_time;
                        entity_change_request.add_new_change::<Velocity>(velocity);
                    }
                }

                // Update position based off of velocity
                let velocity = logic_flow.ecs.get_copy::<Velocity>(*entity).unwrap();
                let mut position = logic_flow.ecs.get_copy::<Position>(*entity).unwrap();
                if nalgebra_glm::length(&velocity.get_velocity()) != 0.0
                {
                    position += velocity * elapsed_time;
                    entity_change_request.add_new_change::<Position>(position);
                    entity_change_request.add_new_change::<HasMoved>(HasMoved);
                }

                if entity_change_request.number_changes() != 0
                {
                    logic_flow.expected_frame_changes.lock().push(FrameChange::EntityChange(vec![EntityChangeInformation::ModifyRequest(entity_change_request)]));
                }

                entity_moved = true;
            }

            if logic_flow.ecs.check_component_written_assume_registered::<VelocityRotation>(*entity)
            {
                let mut entity_change_request = EntityChangeRequest::new(*entity);

                // Update rotation velocity based off of rotation acceleration
                if logic_flow.ecs.check_component_written_assume_registered::<AccelerationRotation>(*entity)
                {
                    let acceleration_rotation = logic_flow.ecs.get_copy::<AccelerationRotation>(*entity).unwrap();
                    let mut velocity_rotation = logic_flow.ecs.get_copy::<VelocityRotation>(*entity).unwrap();
                    if acceleration_rotation.get_rotation_acceleration() != 0.0
                    {
                        velocity_rotation += acceleration_rotation * elapsed_time;
                        entity_change_request.add_new_change::<VelocityRotation>(velocity_rotation);
                    }
                }

                // Update rotation based off of rotation velocity
                let velocity_rotation = logic_flow.ecs.get_copy::<VelocityRotation>(*entity).unwrap();
                let mut rotation = logic_flow.ecs.get_copy::<Rotation>(*entity).unwrap();
                if velocity_rotation.get_rotation() != 0.0
                {
                    rotation += velocity_rotation * elapsed_time;
                    entity_change_request.add_new_change::<Rotation>(rotation);
                    entity_change_request.add_new_change::<HasRotated>(HasRotated);
                }

                if entity_change_request.number_changes() != 0
                {
                    logic_flow.expected_frame_changes.lock().push(FrameChange::EntityChange(vec![EntityChangeInformation::ModifyRequest(entity_change_request)]));
                }

                entity_moved = true;
            }

            if entity_moved && logic_flow.ecs.check_component_written_assume_registered::<CanCauseCollisions>(*entity)
            {
                logic_flow.moved_entities.lock().push(*entity);
            }
        }
    }

    /// Finds collisions between objects, and invokes their collision handlers. All modifications are made
    /// to only the next frame ECS.
    ///
    /// `args` - variables required to perform the entity collision
    pub fn handle_collisions(&self, args: &ExecutionArgs)
    {

        // Note: since moved_entities was created in the update positions function, only entities
        // within the unique world (and linked shared) sections passed into that function can cause a collision

        /*
            Overview of collision algorithm:

            1. Sort moved entities into their world sections
            2. Create vector of world sections from step 1 with relevant information for collisions:

              |world section 0| |world section 1| |world section 2|...

           3. In parallel, find what entities need to be checked for collision
           4. In parallel, execute collision logic
         */

        let moved_entities = self.moved_entities.lock();
        let moved_entities_map: HashSet<&EntityId> = HashSet::from_iter(moved_entities.iter());

        // Find world sections that moved entities are in
        let mut relevant_world_sections: HashMap<UniqueWorldSectionId, Vec<EntityId>> = HashMap::default();

        for entity in &*moved_entities
        {
            match args.bounding_box_tree.entities_index_lookup.get(entity).unwrap()
            {
                WorldSectionLookup::Shared(i) =>
                    {
                        for x in i.to_world_sections().iter().filter_map(|x| *x)
                        {
                            match relevant_world_sections.get_mut(&x)
                            {
                                Some(entities) => entities.push(*entity),
                                None => { relevant_world_sections.insert(x, Vec::new()); }
                            }
                        }
                    },
                WorldSectionLookup::Unique(i) =>
                    {
                        match relevant_world_sections.get_mut(i)
                        {
                            Some(entities) => entities.push(*entity),
                            None => { relevant_world_sections.insert(*i, vec![*entity]); },
                        }
                    }
            }
        }


        // Prepare data to find relevant entities
        struct RelevantEntities
        {
            world_section_id: UniqueWorldSectionId,
            moved_entities: Vec<EntityId>,
            relevant_both_collision_entities: Vec<EntityId>,
            relevant_self_collision_entities: Vec<EntityId>,
        }

        // Use a vector so that later operations can be done in parallel
        let mut sorted_world_section_entities = Vec::with_capacity(relevant_world_sections.len());

        for (world_section_id, entities) in relevant_world_sections
        {
            let relevant_entities = RelevantEntities
            {
                world_section_id,
                moved_entities: entities,
                relevant_both_collision_entities: Vec::new(),
                relevant_self_collision_entities: Vec::new()
            };

            sorted_world_section_entities.push(relevant_entities);
        }


        // Determine what entities need to have collision logic

        // Not using cpu usage reducer since trying to adapt it to using FnMut leads to Rust complaining
        // about the function trait variable not being mutable, but making it mutable means that it cannot
        // be borrowed by Rayon's iterators. So its functionality is emulated here

        let find_relevant_entities = |world_sections: &mut [RelevantEntities]|
            {
                for world_section in world_sections
                {
                    let related_entities = args.bounding_box_tree.find_related_entities(vec![world_section.world_section_id], args.logic_frustum_culler, args.render_frustum_culler);

                    for x in related_entities
                    {
                        match x.location
                        {
                            WorldSectionLookup::Unique(i) =>
                                {
                                    let section_aabb = args.bounding_box_tree.stored_entities_indexes.get(&i).unwrap().aabb;
                                    if distance_to_aabb(&section_aabb, args.camera.get_position()) > 200.0
                                    {
                                        continue;
                                    }
                                },
                            WorldSectionLookup::Shared(i) =>
                                {
                                    let section_aabb = args.bounding_box_tree.shared_section_indexes.get(&i).unwrap().aabb;
                                    if distance_to_aabb(&section_aabb, args.camera.get_position()) > 200.0
                                    {
                                        continue;
                                    }
                                }
                        }

                        // This does not add an entity more than once per the same world section
                        // as this closure is per world section. An entity can be added more than once
                        // for different world sections depending on where the moved entities are
                        for other_entity in x.entities
                        {
                            if moved_entities_map.contains(other_entity)
                            {
                                // Moved entities cause a collision; this branch ensures that a collision between
                                // two moved objects does not cause a moved object to call its own collision function, and
                                // then the other moved objects calls the first object's collision function again
                                world_section.relevant_self_collision_entities.push(*other_entity);
                            }
                            else
                            {
                                world_section.relevant_both_collision_entities.push(*other_entity);
                            }
                        }
                    }
                }
            };

        let mut time_passed_micro = 0;
        let mut number_elements_processed = 0;

        while time_passed_micro < 1000 && number_elements_processed < sorted_world_section_entities.len()
        {
            let time_taken = Instant::now();
            find_relevant_entities(&mut sorted_world_section_entities[number_elements_processed..number_elements_processed + 1]);
            time_passed_micro += time_taken.elapsed().as_micros();
            number_elements_processed += 1;
        }

        sorted_world_section_entities[number_elements_processed..].par_chunks_mut(2).map(|x|
            {
                find_relevant_entities(x);
            }).collect::<()>();

        // Apply collisions as required
        let apply_collision_only_to_self = |this_entity: EntityId, other_entity: EntityId|
            {
                if let Some(this_entity_type) = self.ecs.get_entity_type(this_entity)
                {
                    if let Some(collision_logic) = self.instance_logic.collision_logic.get(&this_entity_type)
                    {
                        let changes = (collision_logic.logic)(this_entity, EntityIdRead::new(other_entity), &self.ecs, args.bounding_box_tree);

                        if !changes.is_empty()
                        {
                            self.expected_frame_changes.lock().push(FrameChange::EntityChange(changes));
                        }
                    }
                }
            };

        let collision_fn = |moved_entities: &[RelevantEntities]|
            {
                for x in moved_entities
                {
                    for moved_entity in &x.moved_entities
                    {
                        let this_aabb = self.ecs.get_copy::<StaticAABB>(*moved_entity).unwrap();

                        for other_entity in &x.relevant_self_collision_entities
                        {
                            if *moved_entity == *other_entity
                            {
                                continue;
                            }

                            let other_entity_aabb = self.ecs.get_ref::<StaticAABB>(*other_entity).unwrap();
                            if this_aabb.intersect(other_entity_aabb)
                            {
                                apply_collision_only_to_self(*moved_entity, *other_entity);
                            }
                        }

                        for other_entity in &x.relevant_both_collision_entities
                        {
                            let other_entity_aabb = self.ecs.get_ref::<StaticAABB>(*other_entity).unwrap();
                            if this_aabb.intersect(other_entity_aabb)
                            {
                                apply_collision_only_to_self(*moved_entity, *other_entity);
                                apply_collision_only_to_self(*other_entity, *moved_entity);
                            }
                        }
                    }
                }
            };

        TimeTakeHistory::apply_to_function(&mut *COLLISION_TIME_HISTORY.lock(), collision_fn, &sorted_world_section_entities);
    }

    /// Performs the onFrame logic for each entity within the specified world sections. All changes to
    /// components made by the logic is written to the next frame's ECS.
    ///
    /// `affected_world_ids` - the world sections that contain entities that which the onFrame logic should be performed
    /// `args` - variables required to perform the entity logic
    pub fn update_logic(&self, affected_world_ids: &Vec<UniqueWorldSectionId>, args: &ExecutionArgs)
    {
        let processed_world_sections: Mutex<HashSet<SharedWorldSectionId>> = Mutex::new(HashSet::default());

        let apply_entity_logic = |ecs: &ECS, entities: &HashSet::<EntityId>, elapsed_time: f32|
            {
                for entity in entities
                {
                    if let Some(entity_type) = self.ecs.get_entity_type(*entity)
                    {
                        if let Some(entity_logic) = self.instance_logic.entity_logic.get(&entity_type)
                        {
                            let changes = (entity_logic.logic)(*entity, ecs, args.bounding_box_tree, elapsed_time);

                            if !changes.is_empty()
                            {
                                self.expected_frame_changes.lock().push(FrameChange::EntityChange(changes));
                            }
                        }

                        if let Some(entity_logic) = self.instance_logic.random_entity_logic.get(&entity_type)
                        {
                            let changes = (entity_logic.logic)(*entity, ecs, args.bounding_box_tree, elapsed_time);

                            if !changes.is_empty()
                            {
                                self.random_frame_changes.lock().push(FrameChange::EntityChange(changes));
                            }
                        }
                    }
                }
            };

        let logic_fn = |world_section_chunk: &[UniqueWorldSectionId]|
            {
                for world_section in world_section_chunk
                {
                    if let Some(entities_in_section) = args.bounding_box_tree.stored_entities_indexes.get(&world_section)
                    {
                        apply_entity_logic(&self.ecs, &entities_in_section.local_entities, args.delta_time);

                        for shared_world_section_index in &entities_in_section.shared_sections_ids
                        {
                            // True if the value was not in map when inserting
                            let look_at_shared_entities = processed_world_sections.lock().insert(*shared_world_section_index);

                            if look_at_shared_entities
                            {
                                match args.bounding_box_tree.shared_section_indexes.get(shared_world_section_index)
                                {
                                    Some(i) =>
                                        {
                                            // See fn that updates position for why this check is done
                                            if args.logic_frustum_culler.aabb_in_view(&i.aabb) ||
                                                args.render_frustum_culler.aabb_in_view(&i.aabb)
                                            {
                                                apply_entity_logic(&self.ecs, &i.entities, args.delta_time);
                                            }
                                        },
                                    // This is a property of the bounding tree- a world section only points to
                                    // a shared section when that shared section exists- if all entities in that
                                    // shared section are removed, the world section no longer points to it
                                    None => unreachable!()
                                }
                            }
                        }
                    }
                }
            };

        TimeTakeHistory::apply_to_function(&mut *LOGIC_TIME_HISTORY.lock(), logic_fn,affected_world_ids);
        apply_entity_logic(&self.ecs, &self.always_execute_entities, args.delta_time);
    }

    /// Updates the bounding box tree based off of the actions performed to an entity that resulted in its position being
    /// changed after the updating functions (movement, collision or on frame logic)
    ///
    /// `bounding_box_tree` - tree that holds all of the entities with a position. This tree is MODIFIED during this function
    /// `model_bank_owner` - owner of the geometric representation of the entities
    /// `camera` - the camera used for rendering
    pub fn update_bounding_box_tree(&mut self, bounding_box_tree: &mut BoundingBoxTree, model_bank_owner: Arc<RwLock<ModelBankOwner>>, camera: &mut Camera, render_flow: &mut RenderFlow)
    {
        let mut model_bank_owner  = model_bank_owner.write();

        let changes = &mut *self.expected_frame_changes.lock();

        let change_args = ChangeArgs
        {
            bounding_box_tree,
            camera,
            ecs: &mut self.ecs,
            model_bank_owner: Some(&mut *model_bank_owner),
            out_of_bounds_logic: &self.instance_logic.out_of_bounds_logic,
            render_flow,
        };

        apply_change(change_args, Some(changes));

        let changes = &mut *self.random_frame_changes.lock();

        let change_args = ChangeArgs
        {
            bounding_box_tree,
            camera,
            ecs: &mut self.ecs,
            model_bank_owner: Some(&mut *model_bank_owner),
            out_of_bounds_logic: &self.instance_logic.out_of_bounds_logic,
            render_flow,
        };

        apply_change(change_args, Some(changes));
    }

    /// Takes all entities that had a component indicating they moved or rotated in the previous frame
    /// and removes those components
    fn reset_has_changed_component(&mut self)
    {
        let entities_that_moved =
            {
                let type_id = [TypeIdentifier::from(TypeId::of::<HasMoved>())];
                self.ecs.get_indexes_for_components(&type_id)
            };
        let entities_that_rotated =
            {
                let type_id = [TypeIdentifier::from(TypeId::of::<HasRotated>())];
                self.ecs.get_indexes_for_components(&type_id)
            };

        for x in entities_that_moved
        {
            self.ecs.remove_component::<HasMoved>(x);
        }

        for x in entities_that_rotated
        {
            self.ecs.remove_component::<HasRotated>(x);
        }
    }

    fn find_always_execute_entities(&mut self, tree: &BoundingBoxTree, visible_sections: &CullResult)
    {
        self.always_execute_entities.clear();

        let always_execute_type = TypeIdentifier::from(TypeId::of::<AlwaysExecuteLogic>());
        let entities_always_logic = self.ecs.get_indexes_for_components(&[always_execute_type]);
        for entity in entities_always_logic
        {
            match tree.entities_index_lookup.get(&entity)
            {
                Some(i) =>
                    {
                        let world_sections = match *i
                        {
                            WorldSectionLookup::Shared(section) => section.to_world_sections().iter().filter_map(|x| *x).collect(),
                            WorldSectionLookup::Unique(section) => vec![section]
                        };

                        let mut entity_already_processed = false;
                        for x in world_sections
                        {
                            if visible_sections.visible_sections_map.contains(&x)
                            {
                                entity_already_processed = true;
                                break;
                            }
                        }

                        if !entity_already_processed
                        {
                            self.always_execute_entities.insert(entity);
                        }
                    },
                None => eprintln!("Unexpected entity for always exeute: {:?}", entity)
            }
        }
    }
}