use glfw::{Action, Key, MouseButton};
use hashbrown::HashMap;

/// Stores the state of the input so that it can be accessed from the draw function
pub struct InputHistory
{
    keys: HashMap<Key, Action>,
    buttons: HashMap<MouseButton, Action>,
}

pub struct CurrentFrameInput
{
    keys: HashMap<Key, Action>,
    buttons: HashMap<MouseButton, Action>,
    latest_cursor_pos: Option<(i32, i32)>
}

impl InputHistory
{
    /// Creates a new empty input history
    pub fn new() -> InputHistory
    {
        InputHistory
        {
            keys: HashMap::default(),
            buttons: HashMap::default(),
        }
    }

    /// Update the state of a key
    ///
    /// `key` - the key that was acted upon
    /// `action` - the action of the key
    pub fn update_key_members(&mut self, key: Key, action: Action)
    {
        self.keys.insert(key, action);
    }

    /// Update the state of a cursor button
    ///
    /// `button` - the button that was acted upon
    /// `action` - the action of the button
    pub fn update_mouse_members(&mut self, button: MouseButton, action: Action)
    {
        self.buttons.insert(button, action);
    }

    /// Checks if the given key is pressed
    ///
    /// `key` - the key to check if it is pressed
    #[allow(dead_code)]
    pub fn is_key_down(&self, key: Key) -> bool
    {
        match self.keys.get(&key)
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
        match self.buttons.get(&button)
        {
            Some(i) => *i == Action::Press,
            None => false
        }
    }
}

impl CurrentFrameInput
{
    pub fn new() -> CurrentFrameInput
    {
        CurrentFrameInput
        {
            keys: HashMap::default(),
            buttons: HashMap::default(),
            latest_cursor_pos: None,
        }
    }

    /// Update the state of a key
    ///
    /// `key` - the key that was acted upon
    /// `action` - the action of the key
    pub fn update_key_members(&mut self, key: Key, action: Action)
    {
        self.keys.insert(key, action);
    }

    /// Update the state of a cursor button
    ///
    /// `button` - the button that was acted upon
    /// `action` - the action of the button
    pub fn update_mouse_members(&mut self, button: MouseButton, action: Action)
    {
        self.buttons.insert(button, action);
    }

    pub fn update_latest_cursor_pos(&mut self, cursor_pos: (i32, i32))
    {
        self.latest_cursor_pos = Some(cursor_pos);
    }

    /// Checks if the given key is pressed
    ///
    /// `key` - the key to check if it is pressed
    #[allow(dead_code)]
    pub fn is_key_down(&self, key: Key) -> bool
    {
        match self.keys.get(&key)
        {
            Some(i) => *i == Action::Press || *i == Action::Repeat,
            None => false
        }
    }

    #[allow(dead_code)]
    pub fn was_key_released(&self, key: Key) -> bool
    {
        match self.keys.get(&key)
        {
            Some(i) => *i == Action::Release,
            None => false
        }
    }

    #[allow(dead_code)]
    pub fn get_latest_cursor_pos(&self) -> Option<(i32, i32)>
    {
        self.latest_cursor_pos
    }

    /// Checks if the given button is pressed
    ///
    /// `button` - the cursor button to check if it is pressed
    #[allow(dead_code)]
    pub fn is_mouse_down(&self, button: MouseButton) -> bool
    {
        match self.buttons.get(&button)
        {
            Some(i) => *i == Action::Press,
            None => false
        }
    }

    pub fn clear(&mut self)
    {
        self.buttons.clear();
        self.keys.clear();
        self.latest_cursor_pos = None;
    }
}