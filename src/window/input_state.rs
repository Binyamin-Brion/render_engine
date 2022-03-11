use glfw::{Action, Key, MouseButton};
use hashbrown::HashMap;

/// Stores the state of the input so that it can be accessed from the draw function
pub struct InputHistory
{
    values: HashMap<i32, Action>
}

impl InputHistory
{
    /// Creates a new empty input history
    pub fn new() -> InputHistory
    {
        InputHistory
        {
            values: HashMap::default()
        }
    }

    /// Update the state of a key
    ///
    /// `key` - the key that was acted upon
    /// `action` - the action of the key
    pub fn update_key_members(&mut self, key: Key, action: Action)
    {
        self.values.insert(key as i32, action);
    }

    /// Update the state of a cursor button
    ///
    /// `button` - the button that was acted upon
    /// `action` - the action of the button
    pub fn update_mouse_members(&mut self, button: MouseButton, action: Action)
    {
        self.values.insert(button as i32, action);
    }

    /// Checks if the given key is pressed
    ///
    /// `key` - the key to check if it is pressed
    #[allow(dead_code)]
    pub fn is_key_down(&self, key: Key) -> bool
    {
        match self.values.get(&(key as i32))
        {
            Some(i) => *i == Action::Press || *i == Action::Repeat,
            None => false
        }
    }

    /// Checks if the given button is pressed
    ///
    /// `button` - the cursor button to check if it is pressed
    #[allow(dead_code)]
    pub fn is_mouse_down(&self, button: MouseButton) -> bool
    {
        match self.values.get(&(button as i32))
        {
            Some(i) => *i == Action::Press,
            None => false
        }
    }
}