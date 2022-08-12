use serde::{Serialize, Deserialize};
use crate::exports::camera_object::SerializableCameraInfo;
use crate::objects::entity_change_request::EntityChangeInformation;

/// Represents the type of state change that can occur in a frame
#[derive(Clone, Serialize, Deserialize)]
pub enum FrameChange
{
    CameraViewChange(SerializableCameraInfo), // f32 = Time delta
    CameraStationary,
    DeltaTime(f32),
    DrawDistancesChange(f32, f32, f32), // Near, Far, FOV
    WindowDimensionsChange((i32, i32)), // Width, Height
    EntityChange(Vec<EntityChangeInformation>),
    EndFrameChange,
}

/// Represents the all of the changes that occur in a single frame
#[derive(Serialize, Deserialize)]
pub struct ChangeHistory
{
    pub changes: Option<Vec<FrameChange>>,
    pub timestamp: u64,
    pub last_thread_to_access: SerializableThreadId
}

/// A thread ID that can be written to disk
#[derive(Eq, PartialEq, Serialize, Deserialize)]
pub struct SerializableThreadId
{
    id: u32,
}

impl SerializableThreadId
{
    /// Creates a new serializable thread ID
    ///
    /// `id` - the ID of the thread
    pub const fn new(id: u32) -> SerializableThreadId
    {
        SerializableThreadId{ id }
    }
}

impl ChangeHistory
{
    /// Creates a new empty state change history
    pub fn new(thread_id: SerializableThreadId) -> ChangeHistory
    {
        ChangeHistory
        {
            last_thread_to_access: thread_id,
            timestamp: 0,
            changes: None,
        }
    }
}