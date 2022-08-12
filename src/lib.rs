use std::mem::swap;
use std::sync::Arc;
use std::{io, thread};
use std::time::Duration;
use lazy_static::lazy_static;
use parking_lot::{Condvar, FairMutex, Mutex};
use crate::exports::load_models::UserUploadInformation;
use crate::helper_things::environment::get_debug_logs_folder;
use crate::helper_things::game_loader::LoadParam;
use crate::helper_things::round_robin_indexer::ArrayIndexer;
use crate::threads::history_thread::{HistoryInputArgs, store_history, StoredHistoryState, write_to_disk};
use crate::threads::public_common_structures::{ChangeHistory, SerializableThreadId};
use crate::threads::render_thread::{render_world, RenderInputArgs};

pub mod exports;
pub mod objects;
pub mod prelude;
pub mod window;
pub mod world;
mod culling;
mod flows;
pub mod helper_things;
mod models;
mod render_components;
mod render_system;
mod threads;

const EXIT_GRACEFULLY_COUNT: u64 = u64::MAX - 1;
const FAILURE_COUNT: u64 = u64::MAX;

pub const MONITOR_THREAD_ID: SerializableThreadId = SerializableThreadId::new(0);
pub const RENDER_THREAD_ID: SerializableThreadId = SerializableThreadId::new(1);
pub const HISTORY_THREAD_ID: SerializableThreadId = SerializableThreadId::new(2);

lazy_static!
{
    pub static ref HISTORY_THREAD_SUCCESS_COUNT: FairMutex<u64> = FairMutex::new(0);
    pub static ref RENDER_THREAD_SUCCESS_COUNT: FairMutex<u64> = FairMutex::new(0);
}

pub type FrameVectors = Arc<[Mutex<ChangeHistory>; 2]>;

pub fn launch_render_system(user_load_info: UserUploadInformation) {

    std::panic::set_hook(Box::new(|info|
        {
            if let Some(error_location) = info.location()
            {
                if error_location.file().contains("history_thread")
                {
                    *HISTORY_THREAD_SUCCESS_COUNT.lock() = FAILURE_COUNT;
                }

                if error_location.file().contains("render_thread")
                {
                    *RENDER_THREAD_SUCCESS_COUNT.lock() = FAILURE_COUNT;
                }
            }

            println!("{}", info);
        }));

    let frame_vectors = Arc::new(
        [
            Mutex::new(ChangeHistory::new(RENDER_THREAD_ID)),
            Mutex::new(ChangeHistory::new(HISTORY_THREAD_ID))
        ]);
    let history_state = Arc::new(Mutex::new(StoredHistoryState::new()));

    let history_condvar = Arc::new(Condvar::new());
    let render_condvar = Arc::new(Condvar::new());

    let mut history_count = *HISTORY_THREAD_SUCCESS_COUNT.lock();
    let mut render_count = *RENDER_THREAD_SUCCESS_COUNT.lock();

    let debug = user_load_info.is_debugging;

    if !debug
    {
        // *********************************************************************************************
        //       Wait for the history thread to be ready to execute
        // *********************************************************************************************

        let history_args = HistoryInputArgs
        {
            frame_vectors: frame_vectors.clone(),
            indexer: ArrayIndexer::<2>::new(0),
            history_condvar: history_condvar.clone(),
            render_condvar: render_condvar.clone(),
            state: history_state.clone()
        };

        thread::spawn(move ||
            {
                store_history(history_args);
            });

        loop
        {
            match wait_for_history_thread_to_launch(60)
            {
                WaitAction::Continue => break,
                WaitAction::Quit=> std::process::exit(0),
                WaitAction::ContinueWaiting => {}
            }
        }
    }

    // *********************************************************************************************
    //       Wait for the render thread to be ready to execute
    //*********************************************************************************************

    let render_args = RenderInputArgs
    {
        frame_vectors: frame_vectors.clone(),
        indexer: ArrayIndexer::<2>::new(1),
        history_condvar: history_condvar.clone(),
        render_condvar: render_condvar.clone(),
        state: history_state.clone(),
    };

    let render_thread = thread::spawn(move ||
        {
            if debug
            {
                let load_param = LoadParam
                {
                    initial_camera: get_debug_logs_folder().join("initial_camera.txt"),
                    gameplay_history: get_debug_logs_folder().join("gameplay_history.txt"),
                    byte_lookup: get_debug_logs_folder().join("gameplay_byte_lookup.txt"),
                };

                render_world(render_args, user_load_info, Some(load_param));
            }
            else
            {
                render_world(render_args, user_load_info, None);
            }
        });

    loop
    {
        match wait_for_render_thread_to_launch(60)
        {
            WaitAction::Continue => break,
            WaitAction::Quit => std::process::exit(0),
            WaitAction::ContinueWaiting => {}
        }
    }

    if !debug
    {

        // *********************************************************************************************
        //     Periodically monitor state of the program to check if an error occurred or if the user
        //     has requested to exit the game
        // *********************************************************************************************

        loop
        {
            std::thread::sleep(Duration::from_secs(1));

            if *RENDER_THREAD_SUCCESS_COUNT.lock() == EXIT_GRACEFULLY_COUNT
            {
                break;
            }

            match check_for_errors(history_count, render_count)
            {
                WaitAction::Continue => {},
                _ => break
            }

            history_count = *HISTORY_THREAD_SUCCESS_COUNT.lock();
            render_count = *RENDER_THREAD_SUCCESS_COUNT.lock();
        }

        let mut args = StoredHistoryState::new();
        swap(&mut args, &mut *history_state.lock());
        write_to_disk(args );
    }
    else
    {
        render_thread.join().unwrap_or_else(|_| panic!("Failed to join render thread"));
    }
}

fn wait_for_render_thread_to_launch(max_timeout_sec: u64) -> WaitAction
{
    let mut time_waited_sec = 0;
    let sleep_internal_sec = 1;

    while time_waited_sec < max_timeout_sec
    {
        match *RENDER_THREAD_SUCCESS_COUNT.lock()
        {
            1 => return WaitAction::Continue,
            FAILURE_COUNT => return user_handle_error("Failed to initialize render"),
            _ =>
                {
                    thread::sleep(Duration::from_secs(sleep_internal_sec));
                    time_waited_sec += sleep_internal_sec;
                }
        }
    }

    return if *RENDER_THREAD_SUCCESS_COUNT.lock() == FAILURE_COUNT
    {
        user_handle_error("Failed to initialize render")
    }
    else
    {
        user_handle_error("Taking an unexpected amount of time to load world")
    }
}

fn wait_for_history_thread_to_launch(max_timeout_sec: u64) -> WaitAction
{
    let mut time_waited_sec = 0;
    let sleep_internal_sec = 1;

    while time_waited_sec < max_timeout_sec
    {
        match *HISTORY_THREAD_SUCCESS_COUNT.lock()
        {
            1 => return WaitAction::Continue,
            FAILURE_COUNT => return user_handle_error("Failed to initialize history"),
            _ =>
                {
                    thread::sleep(Duration::from_secs(sleep_internal_sec));
                    time_waited_sec += sleep_internal_sec;
                }
        }
    }

    return if *HISTORY_THREAD_SUCCESS_COUNT.lock() == FAILURE_COUNT
    {
        user_handle_error("Failed to initialize history")
    }
    else
    {
        user_handle_error("Taking an unexpected amount of time to load history thread")
    }
}

fn user_handle_error(message: &str) -> WaitAction
{
    println!("An error occurred: {}. Would you like to continue waiting to see if error goes away? (y|n)", message);

    loop
    {
        let mut user_response = String::new();
        io::stdin().read_line(&mut user_response).expect("Unable to read response");

        match &user_response[0..1]
        {
            "y" => return WaitAction::ContinueWaiting,
            "n" => return WaitAction::Quit,
            _ => println!("Invalid input"),
        }
    }
}

fn check_for_errors(history_count: u64, render_count: u64) -> WaitAction
{
    let error_history_thread = !(*HISTORY_THREAD_SUCCESS_COUNT.lock() > history_count);
    let error_render_thread = !(*RENDER_THREAD_SUCCESS_COUNT.lock() > render_count);

    return if error_history_thread
    {
        println!("Error history thread");
        WaitAction::Quit
    }
    else if error_render_thread
    {
        println!("Error render thread");
        WaitAction::Quit
    }
    else
    {
        WaitAction::Continue
    }
}

enum WaitAction
{
    Continue,
    ContinueWaiting,
    Quit,
}