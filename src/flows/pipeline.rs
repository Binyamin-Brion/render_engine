use std::sync::Arc;
use std::time::Instant;
use parking_lot::RwLock;
use crate::exports::camera_object::Camera;
use crate::exports::load_models::{AddInstanceFunction, InstanceLogic};
use crate::exports::rendering::LevelOfView;
use crate::flows::logic_flow::{ExecutionArgs, LogicFlow};
use crate::flows::render_flow::{RenderArguments, RenderFlow};
use crate::flows::shared_constants::WORLD_SECTION_LENGTH;
use crate::helper_things::game_loader::GameLoadResult;
use crate::{LoadParam, StoredHistoryState};
use crate::culling::logic_frustum_culler::LogicFrustumCuller;
use crate::culling::render_frustum_culler::RenderFrustumCuller;
use crate::flows::visible_world_flow::VisibleWorldFlow;
use crate::helper_things::entity_change_helpers::{apply_change, ChangeArgs};
use crate::models::model_definitions::ModelId;
use crate::models::model_storage::{LoadModelInfo, ModelBankOwner};
use crate::render_system::render_system::RenderSystem;
use crate::render_system::system_information::DrawFunction;
use crate::threads::public_common_structures::FrameChange;
use crate::window::input_state::InputHistory;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;

type LastFrame = bool;

/// Stores and control the flow of logically handling entities and rendering them
pub struct Pipeline
{
    model_bank_owner: Arc<RwLock<ModelBankOwner>>,
    bounding_box_tree: BoundingBoxTree,
    logic_flow: LogicFlow,
    render_flow: RenderFlow,

    debug_changes: Vec<FrameChange>,
    frame_indexes: Vec<usize>,
    current_frame_index: usize,

}

impl Pipeline
{
    /// Creates a new pipeline to control logic and render flow
    pub fn new(render_systems: Vec<RenderSystem>, (tree_outline_length, tree_atomic_length): (u32, u32),
               instance_logic: InstanceLogic, level_of_views: Vec<LevelOfView>, window_dimensions: (i32, i32), shadow_draw_fn: DrawFunction) -> Pipeline
    {
        *WORLD_SECTION_LENGTH.lock() = tree_atomic_length;

        Pipeline
        {
            model_bank_owner: Arc::new(RwLock::new(ModelBankOwner::new(render_systems.len()))),
            bounding_box_tree: BoundingBoxTree::new(tree_outline_length, tree_atomic_length),
            logic_flow: LogicFlow::new(instance_logic),
            render_flow: RenderFlow::new(render_systems, level_of_views, window_dimensions, shadow_draw_fn),
            debug_changes: Vec::new(),
            frame_indexes: Vec::new(),
            current_frame_index: 0,
        }
    }

    pub fn new_from_file(load_param: LoadParam, render_systems: Vec<RenderSystem>,
                         level_of_views: Vec<LevelOfView>, window_dimensions: (i32, i32),
                         shadow_draw_fn: DrawFunction, instance_logic: InstanceLogic) -> (Pipeline, Arc<RwLock<Camera>>)
    {
        let loaded_state = GameLoadResult::load(load_param);

        let mut frame_indexes = Vec::new();

        for (index, x) in loaded_state.changes.iter().enumerate()
        {
            match x
            {
                FrameChange::EndFrameChange =>  frame_indexes.push(index),
                _ => {}
            }
        }

        let created_state = (
            Pipeline
            {
                model_bank_owner: Arc::new(RwLock::new(ModelBankOwner::new(render_systems.len()))),
                bounding_box_tree: loaded_state.tree,
                logic_flow: LogicFlow::new_from_loaded_state(loaded_state.ecs, instance_logic),
                render_flow: RenderFlow::new(render_systems, level_of_views, window_dimensions, shadow_draw_fn),
                debug_changes: loaded_state.changes,
                frame_indexes,
                current_frame_index: 0,
            },
            Arc::new(RwLock::new(loaded_state.camera))
        );

        *WORLD_SECTION_LENGTH.lock() = created_state.0.bounding_box_tree.atomic_world_section_length();
        created_state
    }

    pub fn update_window_dimension(&mut self, window_dimensions: (i32, i32))
    {
        self.render_flow.update_window_dimension(window_dimensions);
    }

    pub fn synchronize_state(&self, state: &mut StoredHistoryState)
    {
        state.sync_state(&self.logic_flow.ecs, &self.bounding_box_tree, &self.logic_flow.instance_logic.out_of_bounds_logic);
    }

    /// Uploads a new model to the pipeline. Afterwards, instances of the model can be created
    pub fn upload_model<T: Into<String>>(&mut self, model_info: LoadModelInfo<T>) -> ModelId
    {
        let model_id = self.model_bank_owner.write().register_model(&model_info, &mut self.render_flow);
        self.render_flow.register_model_with_render_system(model_info.model_name.into(), model_id, model_info.custom_level_of_view, true);
        model_id
    }

    /// Creates new instances of models that have been uploaded. The function supplied must ONLY add
    /// instances of models specified as a parameter to this function
    pub fn register_model_instances(&mut self, model_id: ModelId, number_instances_to_add: usize, add_function: AddInstanceFunction)
    {
        let created_entities =
        {
            let mut entities = Vec::with_capacity(number_instances_to_add);

            for _ in 0..number_instances_to_add
            {
                let entity = self.logic_flow.ecs.create_entity();
                self.logic_flow.ecs.write_component::<ModelId>(entity, model_id);

                entities.push(entity);
            }

            entities
        };

        let original_aabb = self.model_bank_owner.read().get_model_info(model_id).unwrap().aabb.aabb;
        add_function(&mut self.logic_flow.ecs, created_entities, &mut self.bounding_box_tree, original_aabb);
        self.model_bank_owner.write().register_instances(model_id, number_instances_to_add as u32);

        self.bounding_box_tree.end_of_changes(&self.logic_flow.ecs);
    }

    /// Executes one iteration of the game pipeline. This means that entity logic is handled and the
    /// visible entities are rendered.
    pub fn execute(&mut self, camera: Arc<RwLock<Camera>>, delta_time: f32, input_history: &InputHistory) -> Vec<FrameChange>
    {
        let instant = Instant::now();

        let camera = &mut camera.write();
        let render_frustum_culler = RenderFrustumCuller::new(camera.get_projection_matrix() * camera.get_view_matrix());
        let logic_frustum_culler = LogicFrustumCuller::new(50.0, camera.get_position());

        let mut logically_visible_world_sections =
            VisibleWorldFlow::find_visible_world_ids_map(Arc::new(logic_frustum_culler.clone()), camera.get_position(), 50.0, &self.bounding_box_tree);
        let visible_world_sections=
            VisibleWorldFlow::find_visible_world_ids(Arc::new(render_frustum_culler.clone()), camera.get_position(), camera.get_far_draw_distance(), &self.bounding_box_tree);

        logically_visible_world_sections.extend(visible_world_sections.iter());

        let execution_args = ExecutionArgs
        {
            visible_world_sections: logically_visible_world_sections,
            bounding_box_tree: &mut self.bounding_box_tree,
            model_bank_owner: self.model_bank_owner.clone(),
            delta_time,
            camera: &mut *camera,
            logic_frustum_culler: &logic_frustum_culler,
            render_frustum_culler: &render_frustum_culler,
        };

        let frame_changes = self.logic_flow.execute_logic(execution_args, &mut self.render_flow);

        let render_args = RenderArguments
        {
            visible_world_sections,
            bounding_box_tree: &self.bounding_box_tree,
            ecs: &self.logic_flow.ecs,
            camera: &*camera,
            model_bank_owner: self.model_bank_owner.clone(),
            input_history
        };
        self.render_flow.render(render_args);

        camera.reset_change_param();
        self.bounding_box_tree.clear_changed_static_unique();

       // println!("Time took: {}", instant.elapsed().as_millis());

        frame_changes
    }

    /// Executes an iteration of the game by reading previous game history
    pub fn debug_execute(&mut self, custom_movement: bool, camera: Arc<RwLock<Camera>>, play: bool, input_history: &InputHistory) -> LastFrame
    {
        let camera = &mut *camera.write();
        let logic_frustum_culler = LogicFrustumCuller::new(camera.get_far_draw_distance(), camera.get_position());
        let render_frustum_culler = RenderFrustumCuller::new(camera.get_projection_matrix() * camera.get_view_matrix());

        let mut logically_visible_world_sections =
            VisibleWorldFlow::find_visible_world_ids_map(Arc::new(logic_frustum_culler.clone()), camera.get_position(), 50.0, &self.bounding_box_tree);
        let visible_world_sections=
            VisibleWorldFlow::find_visible_world_ids(Arc::new(render_frustum_culler.clone()), camera.get_position(), camera.get_far_draw_distance(), &self.bounding_box_tree);

        logically_visible_world_sections.extend(visible_world_sections.iter());

        // Only play back history if the user has requested to do so, in order for the user to be able tp
        // pause the playback to observe game state. The frame index check is to prevent out-of-bounds
        // at the end of history playback
        if play && self.frame_indexes.len() != self.current_frame_index
        {
            let begin_index = if self.current_frame_index == 0
            {
                0
            }
            else
            {
                self.frame_indexes[self.current_frame_index - 1]
            };

            let mut delta_time = 0.0;

            for x in begin_index..self.frame_indexes[self.current_frame_index]
            {
                match self.debug_changes[x]
                {
                    FrameChange::EntityChange(_) =>
                        {
                            let mut model_bank_owner = self.model_bank_owner.write();
                            let mut change = vec![self.debug_changes[x].clone()];

                            let change_args = ChangeArgs
                            {
                                bounding_box_tree: &mut self.bounding_box_tree,
                                camera,
                                ecs: &mut self.logic_flow.ecs,
                                model_bank_owner: Some(&mut *model_bank_owner),
                                out_of_bounds_logic: &self.logic_flow.instance_logic.out_of_bounds_logic,
                                render_flow: &mut self.render_flow
                            };

                            apply_change(change_args,Some(&mut change));
                        },
                    FrameChange::CameraViewChange(ref history_camera) =>
                        {
                            if !custom_movement
                            {
                                camera.apply_serialized_data(history_camera);
                            }

                            let execution_args = ExecutionArgs
                            {
                                visible_world_sections: logically_visible_world_sections.clone(),
                                bounding_box_tree: &mut self.bounding_box_tree,
                                model_bank_owner: self.model_bank_owner.clone(),
                                delta_time,
                                camera,
                                logic_frustum_culler: &logic_frustum_culler,
                                render_frustum_culler: &render_frustum_culler.clone()
                            };

                            self.logic_flow.execute_logic(execution_args, &mut self.render_flow);
                        },
                    FrameChange::CameraStationary =>
                        {
                            let execution_args = ExecutionArgs
                            {
                                visible_world_sections: logically_visible_world_sections.clone(),
                                bounding_box_tree: &mut self.bounding_box_tree,
                                model_bank_owner: self.model_bank_owner.clone(),
                                delta_time,
                                camera,
                                logic_frustum_culler: &logic_frustum_culler,
                                render_frustum_culler: &render_frustum_culler.clone()
                            };

                            self.logic_flow.execute_logic(execution_args, &mut self.render_flow);
                        }
                    FrameChange::DeltaTime(recorded_delta_time) =>
                        {
                            delta_time = recorded_delta_time;
                        }
                    FrameChange::DrawDistancesChange(near, far, fov) =>
                        {
                            camera.change_draw_param(near, far, fov);
                        },
                    FrameChange::WindowDimensionsChange(dimensions) =>
                        {
                            camera.account_window_change(dimensions);
                        },
                    FrameChange::EndFrameChange => {}
                }
            }

            println!("{}/{}", self.current_frame_index + 1, self.frame_indexes.len());
            self.current_frame_index += 1;
        }

        let render_args = RenderArguments
        {
            visible_world_sections,
            bounding_box_tree: &self.bounding_box_tree,
            ecs: &self.logic_flow.ecs,
            camera: &*camera,
            model_bank_owner: self.model_bank_owner.clone(),
            input_history
        };
        self.render_flow.render(render_args);
        self.current_frame_index == self.frame_indexes.len() - 1
    }
}