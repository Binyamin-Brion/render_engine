use std::fs;
use std::path::PathBuf;
use crate::exports::camera_object::Camera;
use crate::objects::ecs::ECS;
use crate::threads::public_common_structures::FrameChange;
use crate::world::bounding_box_tree_v2::BoundingBoxTree;

/// Stores the locations of files of game history to use when playing back a different play instance
/// of the game
#[derive(Clone)]
pub struct LoadParam
{
    pub initial_camera: PathBuf,
    pub gameplay_history: PathBuf,
    pub byte_lookup: PathBuf,
}

/// Holds the instances of game data that were stored on disk from a preivous play instance
pub struct GameLoadResult
{
    pub camera: Camera,
    pub ecs: ECS,
    pub tree: BoundingBoxTree,
    pub changes: Vec<FrameChange>,
}

impl GameLoadResult
{
    /// Load a previous play instance so that it can be replayed
    ///
    /// `load_param` - stores the locations of files with previous play instance data
    pub fn load(load_param: LoadParam) -> GameLoadResult
    {
        let initial_camera = fs::read(&load_param.initial_camera).unwrap();
        let camera: Camera = bincode::deserialize(&initial_camera).unwrap();

        let gameplay_history = fs::read(&load_param.gameplay_history).unwrap();
        // This file stores what bytes to read of the gameplay file to extract the correct
        // contents of those files
        let history_lookup = fs::read_to_string(&load_param.byte_lookup).unwrap();
        let byte_lookup = history_lookup.split('\n').filter(|x| *x != "\n").collect::<Vec<&str>>();

        let mut iter = byte_lookup.iter();
        let mut bytes_processed = 0_usize;

        // Read the part of the gameplay file that stores the ECS
        let ecs_offset = iter.next().unwrap();
        let bytes_to_read = ecs_offset.parse::<usize>().unwrap();
        let ecs: ECS = bincode::deserialize(&gameplay_history[bytes_processed..bytes_processed + bytes_to_read]).unwrap();
        bytes_processed += bytes_to_read;

        // Read the part of the gameplay file that stores the bounding box tree
        let tree_offset = iter.next().unwrap();
        let bytes_to_read = tree_offset.parse::<usize>().unwrap();
        let tree: BoundingBoxTree = bincode::deserialize(&gameplay_history[bytes_processed..bytes_processed + bytes_to_read]).unwrap();
        bytes_processed += bytes_to_read;

        // Read the part of the gameplay file that stores frame changes
        let mut changes = Vec::new();
        let number_changes = iter.len() - 1;

        for change_offset in iter.take(number_changes)
        {
            let bytes_to_read = change_offset.parse::<usize>().unwrap();
            let change: FrameChange = bincode::deserialize(&gameplay_history[bytes_processed..bytes_processed + bytes_to_read]).unwrap();
            bytes_processed += bytes_to_read;
            changes.push(change);
        }

        GameLoadResult{ camera, ecs, tree, changes }
    }
}