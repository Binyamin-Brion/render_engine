use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
use glfw::Key::{Escape, Insert, Right, Up};
use hashbrown::HashMap;
use parking_lot::{Condvar, Mutex, MutexGuard};
use crate::{ArrayIndexer, ChangeHistory, EXIT_GRACEFULLY_COUNT, FAILURE_COUNT, FrameVectors,
            get_debug_logs_folder, HISTORY_THREAD_SUCCESS_COUNT, LoadParam, RENDER_THREAD_ID,
            RENDER_THREAD_SUCCESS_COUNT, StoredHistoryState, UserUploadInformation};
use crate::exports::load_models::RenderSystemType;
use crate::exports::logic_components::RenderSystemIndex;
use crate::exports::rendering::LevelOfView;
use crate::exports::user_focused_entities::user_type_identifier;
use crate::flows::pipeline::Pipeline;
use crate::helper_things::environment::get_asset_folder;
use crate::models::model_storage::LoadModelInfo;
use crate::prelude::default_render_system::{create_default_render_system, create_level_of_views};
use crate::threads::private_common_structures::{CAMERA, DELTA_TIME};
use crate::threads::public_common_structures::FrameChange;
use crate::window::gl_window::{GLWindow, GLWindowBuilder};

pub struct RenderInputArgs
{
    pub frame_vectors: FrameVectors,
    pub indexer: ArrayIndexer<2>,
    pub history_condvar: Arc<Condvar>,
    pub render_condvar: Arc<Condvar>,
    pub state: Arc<Mutex<StoredHistoryState>>,
}

#[derive(Eq, PartialEq)]
enum CurrentMode
{
    Run,
    Debug,
    DCustomMovement,
    OnePastLastFrame,
    OnePastLaseFramePause,
}

/// Launches the render thread
///
/// `args` - the structure holding variables required to execute the render thread
/// `debug_mode` - optional information indicating to load a save state, launching render thread in a debug mode
pub fn render_world(mut args: RenderInputArgs, mut user_load_info: UserUploadInformation, debug_mode: Option<LoadParam>)
{
    let mut current_mode = match debug_mode
    {
        Some(_) => CurrentMode::Debug,
        None => CurrentMode::Run,
    };

    let mut window = GLWindowBuilder::new(user_load_info.window_resolution)
        .with_forced_fps(user_load_info.max_fps)
        .with_window_resolution(user_load_info.window_resolution)
        .build()
        .unwrap();

    *CAMERA.write() = user_load_info.initial_camera;

    let mut render_systems = Vec::new();
    let mut render_systems_with_sky_boxes = Vec::new();
    let mut render_system_map = HashMap::new();
    let mut no_light_source_cutoff = 0.0;
    let mut default_diffuse_factor = 1.0;
    for x in user_load_info.render_systems
    {
        let render_system_index = RenderSystemIndex{ index: render_systems.len() };
        render_system_map.insert(x.render_system_name, render_system_index);

        let render_system = match x.render_system
        {
            RenderSystemType::Default(i) =>
                {
                    no_light_source_cutoff = i.no_light_source_cutoff;
                    default_diffuse_factor = i.default_diffuse_factor;

                    create_default_render_system
                        (
                            i.draw_function, i.draw_light_function, i.draw_transparency_function,
                            i.instance_layout_update_fn, i.level_of_views, i.window_resolution, i.sky_boxes, i.max_count_lights,
                            no_light_source_cutoff, default_diffuse_factor
                        )
                }
            RenderSystemType::Custom(i) => i
        };

        if render_system.will_render_skybox()
        {
            render_systems_with_sky_boxes.push(render_system_index);
        }

        render_systems.push(render_system);
    }

    let shadow_lov = if let Some(shadow_lov) = user_load_info.shadow_render_system_lov
    {
        shadow_lov
    }
    else
    {
        create_level_of_views(CAMERA.read().get_render_distance())
    };

    let mut render_pipeline;
    user_load_info.instance_logic.collision_logic.insert(user_type_identifier(), user_load_info.user_collision_function);
    user_load_info.instance_logic.entity_logic.insert(user_type_identifier(), user_load_info.user_logic_function);

    if let Some(ref load_param) = debug_mode
    {
        let (temp_pipeline, camera) = Pipeline::new_from_file(load_param.clone(), no_light_source_cutoff, default_diffuse_factor,
                                                              render_systems,shadow_lov, window.window.get_size(),
                                                              user_load_info.shadow_draw_fn, user_load_info.shadow_light_draw_fn,
                                                              user_load_info.shadow_transparency_draw_fn,
                                                              user_load_info.instance_logic, user_load_info.user_input_functions);

        *CAMERA.write() = camera.read().clone();
        render_pipeline = temp_pipeline;
    }
    else
    {
        let mut file = File::create(get_debug_logs_folder().join("initial_camera.txt")).unwrap();
        file.write_all(&bincode::serialize(&*CAMERA.read()).unwrap()).unwrap_or_else(|err| panic!("Failed to write initial camera settings to file: {:?}", err));

        render_pipeline = Pipeline::new(render_systems, no_light_source_cutoff, default_diffuse_factor,
                                        (16_384, user_load_info.world_section_length),
                                        user_load_info.instance_logic,
                                        shadow_lov, window.window.get_size(), user_load_info.shadow_draw_fn,
                                        user_load_info.shadow_light_draw_fn, user_load_info.shadow_transparency_draw_fn,
                                        user_load_info.user_input_functions, user_load_info.register_instance_function);
    }

    if current_mode == CurrentMode::Run
    {
        render_pipeline.register_user_entity(CAMERA.read().get_position(), user_load_info.user_original_aabb);
    }

    let mut loaded_models = HashMap::new();
    for x in user_load_info.load_models
    {
        let render_system_index = match render_system_map.get(&x.render_system_index)
        {
            Some(i) => *i,
            None => panic!("Unable to find a render system with the name: {}", x.render_system_index)
        };

        let load_info = LoadModelInfo
        {
            model_name: x.model_name.clone(),
            render_system_index,
            location: x.location,
            custom_level_of_view: None,
            model_texture_dir: user_load_info.model_texture_dir.clone(),
            solid_colour_texture: x.solid_colour_texture
        };

        loaded_models.insert(x.model_name, render_pipeline.upload_model(load_info));
    }

    for x in render_systems_with_sky_boxes
    {
        let load_info = LoadModelInfo
        {
            model_name: "skyBox".to_string(),
            render_system_index: x,
            location: vec![get_asset_folder().join("models/skyBox.obj")],
            custom_level_of_view: Some(vec![LevelOfView{ min_distance: 0.0, max_distance: f32::MAX }]),
            model_texture_dir: user_load_info.model_texture_dir.clone(),
            solid_colour_texture: None,
        };

        render_pipeline.upload_model(load_info);
    }

    if current_mode == CurrentMode::Run
    {
        for x in user_load_info.load_instances
        {
            let model_id = match loaded_models.get(&x.model_name)
            {
                Some(i) => *i,
                None => panic!("Unable to find a model with the name: {}", x.model_name)
            };

            render_pipeline.register_model_instances(model_id, x.num_instances, x.upload_fn);
        }

        let render_system_index = match render_system_map.get("default")
        {
            Some(i) => *i,
            None => panic!("Unable to find a render system with the name: default")
        };
        render_pipeline.create_user_entity_instance(render_system_index);
    }

    let error_message = unsafe { std::ffi::CStr::from_ptr(gl::GetString(gl::VENDOR) as *const i8).to_str().unwrap() };
    println!("Company: {}", error_message);

    unsafe
        {
            gl::Enable(gl::DEPTH_TEST);
            gl::Enable(gl::STENCIL_TEST);
        }

    render_pipeline.synchronize_state(&mut *args.state.lock());

    // Tell monitoring thread that render thread has initialized everything successfully
    *RENDER_THREAD_SUCCESS_COUNT.lock() = 1;

    let time_keeper = Instant::now();
    let mut last_frame_time_keeper = Instant::now();
    let mut first_frame = true;

    let mut play = false;

    while !window.should_window_close()
    {
        update_delta_time(first_frame, &mut last_frame_time_keeper);

        // The change lock must be released before the notify_all is called; otherwise the call will
        // have no effect. This could lead to the history thread to keep waiting (depending if the condvar
        // in history will attempt to keep reacquiring the lock after waking up and finding it initially
        // locked. Better to not take that risk). Hence inner scope, to take advantage of RAII
        {
            let mut change_lock = args.frame_vectors[args.indexer.index()].lock();
            wait_until_frame_change_available(&mut change_lock, &args.render_condvar, debug_mode.is_some());

            window.handle_events();
            handle_window_size_update(&window, &mut render_pipeline);
            handle_user_input(&mut window, &mut current_mode, &mut play);

            render_scene(&mut change_lock, &mut window, &mut render_pipeline, &mut current_mode, &mut play);

            change_lock.timestamp = time_keeper.elapsed().as_secs();
            change_lock.last_thread_to_access = RENDER_THREAD_ID;
        }

        if *HISTORY_THREAD_SUCCESS_COUNT.lock() == FAILURE_COUNT
        {
            return;
        }

        // This is called ASAP when lock is no longer needed and it is known history thread is still working
        args.history_condvar.notify_all();

        args.indexer = args.indexer.increment();
        *RENDER_THREAD_SUCCESS_COUNT.lock() += 1;
        first_frame = false;
    }
}

/// Stores how much time has passed since the last iteration of the render loop
///
/// `first_frame` - boolean variable indicating if this is the first iteration of the render loop
/// `last_frame_time_keeper` - the instant variable holding the render loop's time stamp
fn update_delta_time(first_frame: bool, last_frame_time_keeper: &mut Instant)
{
    if first_frame
    {
        // Assume first frame executed really fast. Setting a time of 0 causes logic errors when
        // executing entity rotation kinematics, which relies on frame time
        *DELTA_TIME.write() = 0.001;
    }
    else
    {
        // Store delta time as seconds, but query using milliseconds to ensure sub-second accuracy
        *DELTA_TIME.write() = last_frame_time_keeper.elapsed().as_millis() as f32 / 1000.0;
    }
    *last_frame_time_keeper = Instant::now();
}

/// Waits until the instance of the structure holding the frame changes is available for this thread
///
/// `change_lock` - mutex lock to the structure that holds changes made in the current frame
/// `render_condvar` - condition variable that this thread waits on while the frame change is not
///                     available
/// `debug_mode` - boolean variable indicating if engine was launched in a debug mode
fn wait_until_frame_change_available(mut change_lock: &mut MutexGuard<ChangeHistory>, render_condvar: &Condvar, debug_mode: bool)
{
    // No frame changes are being stored in debugging so no need to wait. This check is included here
    // to make the main render loop cleaner

    while change_lock.last_thread_to_access == RENDER_THREAD_ID && !debug_mode
    {
        if !render_condvar.wait_for(&mut change_lock, Duration::from_secs(1)).timed_out()
        {
            break;
        }

        if *HISTORY_THREAD_SUCCESS_COUNT.lock() == FAILURE_COUNT
        {
            return;
        }
    }
}

/// Stores windows updates in the game history and let's the rendering pipeline know of this change
/// so that it can change the viewport
///
/// `window` - the window being rendered, that was resized
/// `render_pipeline` - the pipeline used for rendering
fn handle_window_size_update(window: &GLWindow, render_pipeline: &mut Pipeline)
{
    if let Some(new_current_dimensions) = window.get_latest_window_dimensions()
    {
        CAMERA.write().account_window_change(new_current_dimensions);
    }

    if let Some(new_window_dimensions) = window.get_latest_window_dimensions()
    {
        render_pipeline.update_window_dimension(new_window_dimensions);
    }
}

/// Execute the appropriate logic based off the user input and the current mode the engine is
/// running in
///
/// `window` - the window being rendered to, that holds the user input
/// `current_mode` - the mode the engine is running in
/// `play` - variable that holds whether the engine should be replaying history when the engine is
///          in debug mode
fn handle_user_input(window: &mut GLWindow, current_mode: &mut CurrentMode, play: &mut bool)
{
    match current_mode
    {
        CurrentMode::Debug  =>
            {
                if window.get_current_input().is_key_down(Escape)
                {
                    *current_mode = CurrentMode::DCustomMovement;
                }

                if window.get_current_input().is_key_down(Right)
                {
                    *play = true;
                }

                if window.get_current_input().was_key_released(Right)
                {
                    *play = false;
                }
            },
        CurrentMode::OnePastLastFrame =>
            {
                if window.get_current_input().is_key_down(Escape)
                {
                    window.set_window_close(); *RENDER_THREAD_SUCCESS_COUNT.lock() = EXIT_GRACEFULLY_COUNT;
                }

                if window.get_current_input().is_key_down(Up)
                {
                    *play = true;
                }
            },
        CurrentMode::OnePastLaseFramePause =>
            {
                if window.get_current_input().is_key_down(Escape)
                {
                    window.set_window_close(); *RENDER_THREAD_SUCCESS_COUNT.lock() = EXIT_GRACEFULLY_COUNT;
                }

                if window.get_current_input().is_key_down(Right)
                {
                    *current_mode = CurrentMode::Run;
                }
            },
        CurrentMode::DCustomMovement =>
            {
                if window.get_current_input().is_key_down(Escape)
                {
                    window.set_window_close();
                    *RENDER_THREAD_SUCCESS_COUNT.lock() = EXIT_GRACEFULLY_COUNT;
                }

                if window.get_current_input().is_key_down(Insert)
                {
                    *current_mode = CurrentMode::Debug;
                }

                if window.get_current_input().is_key_down(Right)
                {
                    *play = true;
                }

                if window.get_current_input().was_key_released(Right)
                {
                    *play = false;
                }
            }
        CurrentMode::Run =>
            {
                if window.get_current_input().is_key_down(Escape)
                {
                    window.set_window_close();
                    *RENDER_THREAD_SUCCESS_COUNT.lock() = EXIT_GRACEFULLY_COUNT;
                }
            }
    }
}

/// Renders the scene according to the current mode the engine is in
///
/// `change_lock` - mutex lock to the structure that holds changes made in the current frame
/// `window` - the window being rendered to
/// `render_pipeline` - the pipeline used for rendering
/// `current_mode` - the mode the engine in running in
/// `play` - variable that holds whether the engine should be replaying history when the engine is
///          in debug mode
fn render_scene(change_lock: &mut MutexGuard<ChangeHistory>, window: &mut GLWindow, render_pipeline: &mut Pipeline, current_mode: &mut CurrentMode, play: &mut bool)
{
    unsafe
        {
            gl::ClearColor(0.3, 0.4, 0.3, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            match current_mode
            {
                CurrentMode::Debug =>
                    {
                        // The CAMERA was updated based off user input; if the current mode is custom movement,
                        // then the serialized camera data that is being replayed is not used

                        if render_pipeline.debug_execute(*current_mode == CurrentMode::DCustomMovement, CAMERA.clone(), *play, false,
                                                         window.get_input_history(), window.get_current_input(), *DELTA_TIME.read())
                        {
                            // Last frame has been executed. Play is false as there are no more history
                            // frames to execute
                            *current_mode = CurrentMode::OnePastLastFrame;
                            *play = false;
                        }
                    },
                CurrentMode::DCustomMovement =>
                    {
                        if render_pipeline.debug_execute(*current_mode == CurrentMode::DCustomMovement,
                                                         CAMERA.clone(), *play, true, window.get_input_history(),
                                                         window.get_current_input(), *DELTA_TIME.read())
                        {
                            // Last frame has been executed. Play is false as there are no more history
                            // frames to execute
                            *current_mode = CurrentMode::OnePastLastFrame;
                            *play = false;
                        }
                    }
                CurrentMode::OnePastLastFrame =>
                    {
                        // Execute the next frame that would appear after the last stored frame. This is useful
                        // during debugging where the last frame stored is the last frame that did not crash,
                        // and would like to view if changes fixed the crash

                        if *play
                        {
                            // Execute the next frame that would exist after the last stored frame
                            render_pipeline.execute(CAMERA.clone(), *DELTA_TIME.read(),
                                                    window.get_input_history(), window.get_current_input());
                            *current_mode = CurrentMode::OnePastLaseFramePause;
                        }
                        else
                        {
                            // Render the last stored frame over and over; this allows the scene to be observed
                            // without any moving parts
                            render_pipeline.debug_execute(*current_mode == CurrentMode::DCustomMovement, CAMERA.clone(),
                                                          false, false, window.get_input_history(),
                                                          window.get_current_input(), *DELTA_TIME.read());
                        }
                    },
                CurrentMode::OnePastLaseFramePause =>
                    {
                        // This renders the next frame after the last stored frame over and over; allows
                        // the scene to be observed without any moving parts
                        render_pipeline.debug_execute(*current_mode == CurrentMode::DCustomMovement,
                                                      CAMERA.clone(), false, true, window.get_input_history(),
                                                      window.get_current_input(), *DELTA_TIME.read());
                    },
                CurrentMode::Run =>
                    {
                        let mut changes = render_pipeline.execute(CAMERA.clone(),
                                                                  *DELTA_TIME.read(), window.get_input_history(), window.get_current_input());
                        changes.push(FrameChange::EndFrameChange);
                        change_lock.changes = Some(changes);
                    }
            }

            window.swap_buffers();
        }
}