use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};
use glfw::{Action, Context, Glfw, InitError, Key, MouseButton, SwapInterval, Window,
           WindowEvent, WindowHint, WindowMode};
use crate::window::input_state::{CurrentFrameInput, InputHistory};
use crate::window::movement_keys;
use crate::window::movement_keys::MovementKeys;

pub const MIDDLE_BUTTON: MouseButton = MouseButton::Button3;

/// Abstraction of a window that can have OpenGL operations submitted to it
pub struct GLWindow
{
    pub glfw: Glfw,
    pub window: Window,
    input_history: InputHistory,
    current_input_history: CurrentFrameInput,
    events: Receiver<(f64, WindowEvent)>,
    wasd_keys: MovementKeys,
    middle_button_down: bool,
    time_per_frame: Option<i64>,
    instant: Instant,

    latest_cursor_pos: Option<(i32, i32)>,
    latest_window_size: Option<(i32, i32)>
}

/// Possible errors that can result from attempting to create a rendering window
#[derive(Debug)]
pub enum GLFWindowCreationError
{
    GLFWInitFailure(String),
    WindowCreationFailure(String),
}

impl From<InitError> for GLFWindowCreationError
{
    fn from(error: InitError) -> Self
    {
        GLFWindowCreationError::GLFWInitFailure(error.to_string())
    }
}

/// Builder for the rendering window
pub struct GLWindowBuilder
{
    default_window_settings: bool,
    window_title: String,
    fullscreen: bool,
    centre_screen: bool,
    window_resolution: (u32, u32),
    window_position: (u32, u32),
    window_hints: Vec<WindowHint>,
    force_fps: Option<i64>,
}

// These operations should be self-explanatory

impl GLWindowBuilder
{
    pub fn new(window_dimensions: (u32, u32)) -> GLWindowBuilder
    {
        GLWindowBuilder
        {
            default_window_settings: true,
            window_title: String::from(""),
            fullscreen: false,
            centre_screen: true,
            window_resolution: window_dimensions,
            window_position: (0, 0),
            window_hints: Vec::new(),
            force_fps: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_title<T: Into<String>>(&mut self, window_title: T) -> &mut Self
    {
        self.window_title = window_title.into();
        self
    }

    #[allow(dead_code)]
    pub fn with_default_window_settings(&mut self, default_settings: bool) -> &mut Self
    {
        self.default_window_settings = default_settings;
        self
    }

    #[allow(dead_code)]
    pub fn with_centre_screen(&mut self, centre_screen: bool) -> &mut Self
    {
        self.centre_screen = centre_screen;
        self
    }

    #[allow(dead_code)]
    pub fn with_window_resolution(&mut self, resolution: (u32, u32)) -> &mut Self
    {
        self.window_resolution = resolution;
        self
    }

    #[allow(dead_code)]
    pub fn with_window_position(&mut self, position: (u32, u32)) -> &mut Self
    {
        self.window_position = position;
        self
    }

    #[allow(dead_code)]
    pub fn with_window_hints(&mut self, mut hints: Vec<WindowHint>) -> &mut Self
    {
        self.window_hints.append(&mut hints);
        self
    }

    #[allow(dead_code)]
    pub fn as_fullscreen(&mut self, fullscreen: bool) -> &mut Self
    {
        self.fullscreen = fullscreen;
        self
    }

    #[allow(dead_code)]
    pub fn with_forced_fps(&mut self, fps: i64) -> &mut Self
    {
        self.force_fps = Some(fps);
        self
    }

    pub fn build(&self) -> Result<GLWindow, GLFWindowCreationError>
    {
        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)?;

        for x in &self.window_hints
        {
            glfw.window_hint(x.clone());
        }

        let (mut window, events) = match glfw.create_window(self.window_resolution.0, self.window_resolution.1, &self.window_title, WindowMode::Windowed)
        {
            Some((window, events)) => (window, events),
            None => return Err(GLFWindowCreationError::WindowCreationFailure(String::from("Failed to create window")))
        };

        if self.default_window_settings
        {
            window.set_key_polling(true);
            window.set_mouse_button_polling(true);
            window.set_cursor_pos_polling(true);
            window.set_size_polling(true);
            window.make_current();
        }

        if self.force_fps.is_none()
        {
            glfw.set_swap_interval(SwapInterval::Sync(1))
        }

        gl::load_with(|s| window.get_proc_address(s) as *const _);

        if self.fullscreen
        {
            glfw.with_primary_monitor_mut(|_, monitor|
                {
                    if let Some(monitor) = monitor
                    {
                        if let Some(mode) =  monitor.get_video_mode()
                        {
                            window.set_monitor(WindowMode::FullScreen(&monitor), 0, 0, mode.width, mode.height, None);
                        }
                    }
                })
        }
        else if self.centre_screen
        {
            glfw.with_primary_monitor_mut(|_, monitor|
                {
                    if let Some(monitor) = monitor
                    {
                        if let Some(mode) =  monitor.get_video_mode()
                        {
                            let monitor_pos = monitor.get_pos();
                            let window_size = window.get_size();

                            window.set_pos
                            (
                                monitor_pos.0 + (mode.width as i32 - window_size.0) / 2,
                                monitor_pos.1 + (mode.height as i32 - window_size.1) / 2,
                            );
                        }
                    }
                })
        }
        else
        {
            window.set_pos(self.window_position.0 as i32, self.window_position.1 as i32);
        }

        unsafe
            {
                gl::Viewport(0, 0, window.get_size().0, window.get_size().1);

                gl::Enable(gl::DEBUG_OUTPUT);
                gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS); // makes sure errors are displayed synchronously
                gl::DebugMessageCallback(Some(gl_debug_output), std::ptr::null());
                gl::DebugMessageControl(
                    gl::DONT_CARE,
                    gl::DONT_CARE,
                    gl::DONT_CARE,
                    0,
                    std::ptr::null(),
                    gl::TRUE,
                );
            }

        let time_per_frame = match self.force_fps
        {
            Some(i) => Some(1000 / i),
            None => None,
        };

        let window = GLWindow
        {
            glfw, window, events, wasd_keys: MovementKeys::new(),
            current_input_history: CurrentFrameInput::new(), latest_cursor_pos: None, middle_button_down: false,
            time_per_frame, instant: Instant::now(), latest_window_size: None, input_history: InputHistory::new(),
        };

        Ok(window)
    }
}

impl GLWindow
{
    /// Get the status of the movement keys
    #[allow(dead_code)]
    pub fn get_movement_keys(&self) -> &MovementKeys
    {
        &self.wasd_keys
    }

    /// Query if the window should close in the next render loop
    #[allow(dead_code)]
    pub fn should_window_close(&self) -> bool
    {
        self.window.should_close()
    }

    /// Notify that the window should close in the next render loop
    #[allow(dead_code)]
    pub fn set_window_close(&mut self)
    {
        self.window.set_should_close(true);
    }

    pub fn get_input_history(&self) -> &InputHistory
    {
        &self.input_history
    }

    pub fn get_current_input(&self) -> &CurrentFrameInput
    {
        &self.current_input_history
    }

    /// Get the status of the movement keys
    #[allow(dead_code)]
    pub fn get_movement_key_status(&self) -> MovementKeys
    {
        self.wasd_keys.clone()
    }

    /// Get the latest position of the cursor, if any. Thus if a frame contains several mouse movement
    /// events, this function will return the position of the last event
    #[allow(dead_code)]
    pub fn get_latest_cursor_pos(&self) -> Option<(i32, i32)>
    {
        self.latest_cursor_pos
    }

    /// Get the last known change to the window dimensions. Once the history is cleared, this returns
    /// None until a new window dimension change occurs
    #[allow(dead_code)]
    pub fn get_latest_window_dimensions(&self) -> Option<(i32, i32)>
    {
        self.latest_window_size
    }

    /// Checks if the middle button is down
    pub fn middle_button_down(&self) -> bool
    {
        self.middle_button_down
    }

    /// Stores any new input and changes state as required, and deletes old input history
    pub fn handle_events(&mut self)
    {
        self.wait_for_fps();

        self.clear_input_history();

        self.glfw.poll_events();
        for (_, event) in glfw::flush_messages(&self.events)
        {
            match event
            {
                glfw::WindowEvent::Key(Key::W, _, Action::Press, _) =>
                    {
                        self.wasd_keys.set_key_state
                        (
                            movement_keys::KeyID::KeyW,
                            movement_keys::KeyStatus::Pressed,
                        );
                    }
                glfw::WindowEvent::Key(Key::A, _, Action::Press, _) =>
                    {
                        self.wasd_keys.set_key_state
                        (
                            movement_keys::KeyID::KeyA,
                            movement_keys::KeyStatus::Pressed,
                        );
                    }
                glfw::WindowEvent::Key(Key::S, _, Action::Press, _) =>
                    {
                        self.wasd_keys.set_key_state
                        (
                            movement_keys::KeyID::KeyS,
                            movement_keys::KeyStatus::Pressed,
                        );
                    }
                glfw::WindowEvent::Key(Key::D, _, Action::Press, _) =>
                    {
                        self.wasd_keys.set_key_state(
                            movement_keys::KeyID::KeyD,
                            movement_keys::KeyStatus::Pressed,
                        );
                    }
                glfw::WindowEvent::Key(Key::W, _, Action::Release, _) =>
                    {
                        self.wasd_keys.set_key_state
                        (
                            movement_keys::KeyID::KeyW,
                            movement_keys::KeyStatus::Released,
                        );
                    }
                glfw::WindowEvent::Key(Key::A, _, Action::Release, _) =>
                    {
                        self.wasd_keys.set_key_state
                        (
                            movement_keys::KeyID::KeyA,
                            movement_keys::KeyStatus::Released,
                        );
                    }
                glfw::WindowEvent::Key(Key::S, _, Action::Release, _) =>
                    {
                        self.wasd_keys.set_key_state
                        (
                            movement_keys::KeyID::KeyS,
                            movement_keys::KeyStatus::Released,
                        );
                    }
                glfw::WindowEvent::Key(Key::D, _, Action::Release, _) =>
                    {
                        self.wasd_keys.set_key_state
                        (
                            movement_keys::KeyID::KeyD,
                            movement_keys::KeyStatus::Released,
                        );
                    },
                glfw::WindowEvent::MouseButton(MIDDLE_BUTTON, Action::Press, _) =>
                    {
                        self.middle_button_down = true;

                    },
                glfw::WindowEvent::MouseButton(MIDDLE_BUTTON, Action::Release, _) =>
                    {
                        self.middle_button_down = false;

                    },
                glfw::WindowEvent::Size(width, height) =>
                    {
                        unsafe
                            {
                                gl::Viewport(0, 0, width, height);
                            }
                    }
                _ =>
                    {}
            }

            match event
            {
                glfw::WindowEvent::Key(key, _, action, _) =>
                    {
                        self.input_history.update_key_members(key, action);
                        self.current_input_history.update_key_members(key, action);
                    }
                glfw::WindowEvent::MouseButton(button, action, _) =>
                    {
                        self.input_history.update_mouse_members(button, action);
                        self.current_input_history.update_mouse_members(button, action);
                    },
                glfw::WindowEvent::CursorPos(x, y) =>
                    {
                        self.current_input_history.update_latest_cursor_pos((x as i32, y as i32))
                    },
                glfw::WindowEvent::Size(width, height) =>
                    {
                        self.latest_window_size = Some((width, height));
                    }
                _ => {}
            }
        }
    }

    /// Swaps buffers of the rendering window. Call at the end of the frame loop
    pub fn swap_buffers(&mut self)
    {
        self.window.swap_buffers();
    }

    /// Clears history of inputs given by the user
    fn clear_input_history(&mut self)
    {
        self.current_input_history.clear();
        self.latest_cursor_pos = None;
        self.latest_window_size = None;
    }

    /// Limits the FPS to what was specified during the window creation
    fn wait_for_fps(&mut self)
    {
        if let Some(time_per_frame) = self.time_per_frame
        {
            let elapsed_time = self.instant.elapsed().as_millis() as i64;

            let time_to_wait = if elapsed_time > time_per_frame
            {
                0
            }
            else
            {
                (time_per_frame - elapsed_time).max(0)
            };

            thread::sleep(Duration::from_millis(time_to_wait as u64));
        }

        self.instant = Instant::now();
    }
}

extern "system" fn gl_debug_output(
    _source: gl::types::GLenum,
    _type_: gl::types::GLenum,
    id: gl::types::GLuint,
    _: gl::types::GLenum,
    _length: gl::types::GLsizei,
    message: *const gl::types::GLchar,
    _user_param: *mut std::ffi::c_void,
)
{
    let message = unsafe { std::ffi::CStr::from_ptr(message).to_str().unwrap() };
    let gpu_vendor = unsafe { std::ffi::CStr::from_ptr(gl::GetString(gl::VENDOR) as *const i8).to_str().unwrap() };

    if gpu_vendor.contains("Intel") && message.contains("API_ID_SYNC_FLUSH")
    {
        return;
    }

    println!("Debug message ({}): {}", id, message);
}