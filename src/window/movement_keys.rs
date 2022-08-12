/// Holds that status of movement keys. This is used as query if a key is pressed has a delay after
/// the initial press
#[derive(Copy, Clone)]
pub struct MovementKeys
{
    pub keys: [KeyStatus; 4]
}

/// Status of the key being acted upon
#[derive(Copy, Clone, PartialEq)]
pub enum KeyStatus
{
    Pressed,
    Released
}

/// Indexes of the movement keys into the MovementKeys structure
#[repr(usize)]
#[derive(Copy, Clone)]
pub enum KeyID
{
    KeyA = 0,
    KeyD = 1,
    KeyS = 2,
    KeyW = 3
}

impl MovementKeys
{
    /// Create a new instance of the movement keys
    pub fn new() -> MovementKeys
    {
        MovementKeys { keys: [KeyStatus::Released; 4] }
    }

    /// Set the status of a movement key
    pub fn set_key_state(&mut self, key_id: KeyID, state: KeyStatus)
    {
        self.keys[key_id as usize] = state;
    }
}