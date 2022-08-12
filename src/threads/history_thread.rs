use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::mem::swap;
use std::sync::Arc;
use std::time::Duration;
use hashbrown::HashMap;
use parking_lot::{Condvar, Mutex};
use crate::{ArrayIndexer, ChangeHistory, EXIT_GRACEFULLY_COUNT, FAILURE_COUNT, FrameVectors,
            get_debug_logs_folder, HISTORY_THREAD_ID, HISTORY_THREAD_SUCCESS_COUNT, RENDER_THREAD_SUCCESS_COUNT};
use crate::exports::logic_components::OutOfBoundsLogic;
use crate::objects::ecs::{ECS, TypeIdentifier};
use crate::threads::private_common_structures::{CAMERA, DELTA_TIME};
use crate::threads::public_common_structures::FrameChange;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;

/// Variables required for the history thread to operate
pub struct HistoryInputArgs
{
    pub frame_vectors: FrameVectors,
    pub indexer: ArrayIndexer<2>,
    pub history_condvar: Arc<Condvar>,
    pub render_condvar: Arc<Condvar>,
    pub state: Arc<Mutex<StoredHistoryState>>,
}

/// The state of the program that has been recorded
pub struct StoredHistoryState
{
    game_history_ecs: ECS,
    game_history_bounding_box_tree: BoundingBoxTree,
    game_history_changes_to_apply: VecDeque<ChangeHistory>,
    out_of_bounds_logic: HashMap<TypeIdentifier, OutOfBoundsLogic>,
}

impl StoredHistoryState
{
    /// Constructs a new empty recorded state for the program
    pub fn new() -> StoredHistoryState
    {
        StoredHistoryState
        {
            game_history_ecs: ECS::new(),
            game_history_bounding_box_tree: BoundingBoxTree::new(0, 0),
            game_history_changes_to_apply: VecDeque::new(),
            out_of_bounds_logic: HashMap::default(),
        }
    }

    /// Updates the recorded state so that it matches the state given to the function
    ///
    /// `ecs` - the instance of the ECS to sync with
    /// `tree` - the instance of bounding volume tree to sync with
    /// `out_of_bounds_logic` - the instance of the out of bounds logic to sync with
    pub fn sync_state(&mut self, ecs: &ECS, tree: &BoundingBoxTree, out_of_bounds_logic: &HashMap<TypeIdentifier, OutOfBoundsLogic>)
    {
        self.game_history_ecs = ecs.clone();
        self.game_history_bounding_box_tree = tree.clone();
        self.game_history_changes_to_apply.clear();
        self.out_of_bounds_logic = out_of_bounds_logic.clone();
    }
}

/// Records the most recent state changes done by the performance thread
///
/// `args` - structure holding variable required to store history
pub fn store_history(mut args: HistoryInputArgs)
{
    let update_timeout_seconds = Duration::from_secs(5);

    loop
    {
        let mut frame_vector = args.frame_vectors[args.indexer.index()].lock();

        // History thread did a full iteration of frame vectors before render thread could update
        // current frame vector
        while frame_vector.last_thread_to_access == HISTORY_THREAD_ID
        {
            if !args.history_condvar.wait_for( &mut frame_vector, update_timeout_seconds).timed_out()
            {
                break;
            }

            if render_thread_down()
            {
                return;
            }
        }

        let state = &mut args.state.lock();

        // Get the most recent frame changes without doing a copy of the changes- this is done for
        // performance reasons
        let mut frame_changes = ChangeHistory::new(HISTORY_THREAD_ID);
        swap(&mut frame_changes, &mut *frame_vector);

        state.game_history_changes_to_apply.push_back(frame_changes);

        // Check if render thread crashed when it applied the set of changes that this thread will apply
        // at some point in the future
        if render_thread_down()
        {
            // The changes have been written to the history state, which is needed in order to playback
            // the issue
            return;
        }

        frame_vector.last_thread_to_access = HISTORY_THREAD_ID;
        args.indexer = args.indexer.increment();
        *HISTORY_THREAD_SUCCESS_COUNT.lock() += 1;

        // Tell render thread it can overwrite current frame vector if it is waiting to do so
        args.render_condvar.notify_all();
    }
}

/// Determines if the render thread is down, meaning this thread needs to quit
fn render_thread_down() -> bool
{
    // Checking render thread status is done over two operations to prevent deadlock of performing
    // same action when done in one line
    let render_failure = *RENDER_THREAD_SUCCESS_COUNT.lock() == FAILURE_COUNT;
    let render_exit = *RENDER_THREAD_SUCCESS_COUNT.lock() == EXIT_GRACEFULLY_COUNT;

    render_failure || render_exit
}

/// Stores the last known camera status into the recorded history
///
/// `recorded_state` - the variable that holds recorded history of the render engine when not in debug mode
fn store_last_camera_status(recorded_state: &mut StoredHistoryState)
{
    let last_frame_change = ChangeHistory
    {
        changes: Some(vec![
            FrameChange::DeltaTime(*DELTA_TIME.read()),
            FrameChange::CameraViewChange(CAMERA.read().get_serializable_data().clone()),
            FrameChange::EndFrameChange,
        ]),
        timestamp: 0,
        last_thread_to_access: HISTORY_THREAD_ID
    };

    recorded_state.game_history_changes_to_apply.push_back(last_frame_change);
}

/// Writes the stored history, if any, to disk
///
/// `recorded_state` - the state that was stored during the execution of the engine while not in debug mode
pub fn write_to_disk(mut recorded_state: StoredHistoryState)
{
    store_last_camera_status(&mut recorded_state);

    let file = File::create(get_debug_logs_folder().join("gameplay_history.txt")).unwrap();
    let mut buf_writer = BufWriter::new(file);

    let byte_lookup_file = File::create(get_debug_logs_folder().join("gameplay_byte_lookup.txt")).unwrap();
    let mut bytes_written_history = Vec::new();

    let mut attempt_write = |content: &[u8], content_name: &str|
        {
            if buf_writer.write_all(content).is_err()
            {
                std::thread::sleep(Duration::from_secs(5));

                if buf_writer.write_all(content).is_err()
                {
                    panic!("Failed to write: {}", content_name);
                }
            }

            content.len()
        };

    let ecs_string = bincode::serialize(&recorded_state.game_history_ecs).unwrap();
    let bounding_box_tree_string = bincode::serialize(&recorded_state.game_history_bounding_box_tree).unwrap();

    let ecs_bytes = attempt_write(&ecs_string, "ECS contents");
    bytes_written_history.push( ecs_bytes);

    let tree_bytes = attempt_write(&bounding_box_tree_string, "Bounding Box Tree Contents");
    bytes_written_history.push(tree_bytes);

    for x in recorded_state.game_history_changes_to_apply
    {
        if let Some(changes) = x.changes
        {
            for specific_change in changes
            {
                let serialized_change = bincode::serialize(&specific_change).unwrap();

                let change_bytes = attempt_write(&serialized_change, "Frame Change contents");
                bytes_written_history.push(change_bytes);
            }
        }
    }

    buf_writer = BufWriter::new(byte_lookup_file);

    for x in bytes_written_history
    {
        let index_string = x.to_string() + "\n";
        buf_writer.write(index_string.as_bytes()).unwrap_or_else(|err| panic!("Failed to write history to file: {}", err));
    }
}