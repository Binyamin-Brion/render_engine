use nalgebra_glm::{cross, look_at, normalize, ortho, perspective, TMat4, TVec3, vec3};
use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct MovementFactor
{
    pub forwards_backwards: f32,
    pub left_right: f32,
}

/// A camera that provides the perspective from which the 3D world is rendered.
#[derive(Clone, Serialize, Deserialize)]
pub struct Camera
{
    // Core variables
    direction: TVec3<f32>,
    position: TVec3<f32>,
    projection_matrix: TMat4<f32>,
    view_matrix: TMat4<f32>,
    up: TVec3<f32>,

    // Rotation related variables
    pub first_rotation: bool,
    last_rotation_x: i32,
    last_rotation_y: i32,
    max_angle_look_down: f32,
    max_angle_look_up: f32,
    pitch: f32,
    yaw: f32,
    near_draw_distance: f32,
    far_draw_distance: f32,

    // Other variables
    mouse_sensitivity: f32,
    movement_speed_factor: f32,

    pub undefined_state: bool,
    pub window_width: i32,
    pub window_height: i32,
    fov: f32,

    view_matrix_changed: bool,
    draw_param_changed: bool, // near, far, fov
window_dimensions_change: bool,
}

/// Stores data to be serialized about the camera into one package
#[derive(Clone, Serialize, Deserialize)]
pub struct SerializableCameraInfo
{
    position: TVec3<f32>,
    direction: TVec3<f32>,
}

impl Camera
{
    /// Creates a new camera that does not have any sensible values. This is so that an instance
    /// of the camera can be created globally, before code populating it with logical values can
    /// be executed
    pub fn new_undefined() -> Camera
    {
        let mut camera = CameraBuilder::new((1, 1)).build();
        camera.undefined_state = true;
        camera
    }

    /// Updates the camera's position, direction and view_matrix with data read from game history
    ///
    /// `data` - the read game history that contains updated for the camera
    pub fn apply_serialized_data(&mut self, data: &SerializableCameraInfo)
    {
        self.position = data.position;
        self.direction = data.direction;
        self.view_matrix =  look_at(&self.position, &(&self.position + &self.direction), &vec3(0.0, 1.0, 0.0));
    }

    /// Updates internal variables to account for a change in the window size
    ///
    /// `dimensions` - the new dimensions of the window
    pub fn account_window_change(&mut self, dimensions: (i32, i32))
    {
        self.window_width = dimensions.0;
        self.window_height = dimensions.1;

        self.projection_matrix = perspective( self.window_width as f32 / self.window_height as f32,
                                              nalgebra_glm::radians(&nalgebra_glm::vec1(self.fov))[0],
                                              self.near_draw_distance, self.far_draw_distance);

        // This will make it so that the change in window size, and therefore the camera's integral
        // variables have changed, causing them to be stored in the game history. Could replace with
        // self.view_matrix_changed from the storage of the game history perspective, but this
        // provides a new separation as to why the view matrix changed
        self.window_dimensions_change = true;
    }

    /// Change the drawing parameters of the camera
    ///
    /// `near` - the near distance of the camera
    /// `far` - the far distance of the camera
    /// `fov` - the field of view of the camea
    pub fn change_draw_param(&mut self, near: f32, far: f32, fov: f32)
    {
        self.near_draw_distance = near;
        self.far_draw_distance = far;
        self.fov = fov;

        self.projection_matrix = perspective( self.window_width as f32 / self.window_height as f32,
                                              nalgebra_glm::radians(&nalgebra_glm::vec1(self.fov))[0],
                                              self.near_draw_distance, self.far_draw_distance);

        // Will cause these changes to be stored in game history
        self.draw_param_changed = true;
    }

    /// Returns if the view matrix has changed. Used to determine if new camera change
    /// history needs to be stored
    pub fn get_view_matrix_changed(&self) -> bool
    {
        self.view_matrix_changed
    }

    /// Returns if the draw parameters have changed (near, far, fov). Used to determine if new camera change
    /// history needs to be stored
    pub fn get_draw_param_changed(&self) -> bool
    {
        self.draw_param_changed
    }

    /// Returns if the window dimensions have changed. Used to determine if new camera change
    /// history needs to be stored
    pub fn get_window_dimensions_changed(&self) -> bool
    {
        self.window_dimensions_change
    }

    /// Get the FOV of the camera
    pub fn get_fov(&self) -> f32
    {
        self.fov
    }

    /// Get the near draw distance of the camera
    pub fn get_near_draw_distance(&self) -> f32
    {
        self.near_draw_distance
    }

    /// Get the far draw distance of the camera
    pub fn get_far_draw_distance(&self) -> f32
    {
        self.far_draw_distance
    }

    /// Get the window dimensions used to calculate internal variables of the camera
    pub fn get_window_dimensions(&self) -> (i32, i32)
    {
        (self.window_width, self.window_height)
    }

    pub fn get_direction(&self) -> TVec3<f32>
    {
        self.direction
    }

    /// Changes to the camera are marked as not having occurred. Prevents duplicate copies of the
    /// camera's changes from being stored in the game history
    pub fn reset_change_param(&mut self)
    {
        self.draw_param_changed = false;
        self.view_matrix_changed = false;
        self.window_dimensions_change = false;
    }

    pub fn float_position(&mut self, move_factor: MovementFactor, delta_time: f32)
    {
        self.position += self.direction * move_factor.forwards_backwards * delta_time;
        self.position += normalize(&cross(&self.direction, &self.up)) * move_factor.left_right * delta_time;
        self.view_matrix =  look_at(&self.position, &(&self.position + &self.direction), &vec3(0.0, 1.0, 0.0));
        self.view_matrix_changed = true;
    }

    /// Get the projection matrix of the camera.
    pub fn get_projection_matrix(&self) -> TMat4<f32>
    {
        self.projection_matrix
    }

    /// Get the view matrix of the camera.
    pub fn get_view_matrix(&self) -> TMat4<f32>
    {
        self.view_matrix
    }

    /// Get serializable data for the camera
    pub fn get_serializable_data(&self) -> SerializableCameraInfo
    {
        SerializableCameraInfo
        {
            direction: self.direction,
            position: self.position
        }
    }

    /// Get the position of the camera
    pub fn get_position(&self) -> TVec3<f32>
    {
        self.position
    }

    /// Get the render distance of the camera
    pub fn get_render_distance(&self) -> f32
    {
        self.far_draw_distance
    }

    /// Rotates the camera based off of how much the mouse has moved since the last time this function
    /// was called
    ///
    /// `mouse_pos_x` - the position of the cursor in the x-axis
    /// `mouse_pos_y` - the position of the cursor in the y-axis
    pub fn rotate(&mut self, mouse_pos_x: i32, mouse_pos_y: i32)
    {
        if self.first_rotation
        {
            self.last_rotation_x = mouse_pos_x;
            self.last_rotation_y = mouse_pos_y;
            self.first_rotation = false;
        }

        let mut mouse_offset_x = (mouse_pos_x - self.last_rotation_x) as f32;
        let mut mouse_offset_y = (self.last_rotation_y - mouse_pos_y) as f32;

        self.last_rotation_x = mouse_pos_x;
        self.last_rotation_y = mouse_pos_y;

        mouse_offset_x *= self.mouse_sensitivity;
        mouse_offset_y *= self.mouse_sensitivity;

        self.yaw += mouse_offset_x;
        self.pitch += mouse_offset_y;

        if self.pitch > self.max_angle_look_up
        {
            self.pitch = self.max_angle_look_up;
        }

        if self.pitch < self.max_angle_look_down
        {
            self.pitch = self.max_angle_look_down;
        }

        let new_x_direction = self.yaw.to_radians().cos() * self.pitch.to_radians().cos();
        let new_y_direction = self.pitch.to_radians().sin();
        let new_z_direction = self.yaw.to_radians().sin() * self.pitch.to_radians().cos();

        self.direction = normalize(&vec3(new_x_direction, new_y_direction, new_z_direction));

        self.view_matrix =  look_at(&self.position, &(&self.position + &self.direction), &vec3(0.0, 1.0, 0.0));
        self.view_matrix_changed = true;
    }

    pub fn force_hard_position(&mut self, position: TVec3<f32>)
    {
        self.position = position;
        self.view_matrix =  look_at(&self.position, &(&self.position + &self.direction), &vec3(0.0, 1.0, 0.0));
        self.view_matrix_changed = true;
    }
}

/// A builder to provide a cleaner interface to specify values to a created Camera.
pub struct CameraBuilder
{
    window_dimensions: (i32, i32),

    direction: TVec3<f32>,
    fov: f32,
    near_draw_distance: f32,
    far_draw_distance: f32,
    position: TVec3<f32>,
    up: TVec3<f32>,

    // Rotation related variables
    max_angle_look_down: f32,
    max_angle_look_up: f32,
    pitch: f32,
    yaw: f32,

    // Other variables
    pub mouse_sensitivity: f32,
    pub movement_speed_factor: f32,

    // For orthographic cameras
    is_orthographic: bool,
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
}

impl CameraBuilder
{
    /// Constructs s new camera builder with default parameters for the camera
    ///
    /// `window_dimensions` - the dimensions of the window at the time of calling this function
    pub fn new(window_dimensions: (i32, i32)) -> CameraBuilder
    {
        let position = vec3(0.0, 0.0, 40.0);

        let direction = vec3(1.0, 0.0, 0.0);
        let up = vec3(0.0, 1.0, 0.0);
        let fov: f32 = 45.0;
        let near_draw_distance: f32 = 0.1;
        let far_draw_distance: f32 = 100.0;
        let mouse_sensitivity: f32 = 0.1;
        let movement_speed_factor: f32 = 0.5;
        let pitch: f32 = 0.0;
        let yaw: f32 = 0.0;
        let max_angle_look_down: f32 = -89.0;
        let max_angle_look_up: f32 = 89.0;
        let left = 0.0;
        let right = 0.0;
        let top = 0.0;
        let bottom = 0.0;
        let near = 0.0;
        let far = 0.0;
        let is_orthographic = false;

        CameraBuilder
        {
            window_dimensions, direction, fov, position, near_draw_distance, far_draw_distance, up,
            max_angle_look_down, max_angle_look_up,
            pitch, yaw, mouse_sensitivity, movement_speed_factor,
            left, right, top, bottom, near, far, is_orthographic
        }
    }

    /// Builds a new Camera from the supplied argument to the builder. If no value was given for a
    /// specific variable, then the default value will be supplied by the builder
    pub fn build(&self) -> Camera
    {
        let projection_matrix = if self.is_orthographic
        {
            ortho(self.left, self.right, self.bottom, self.top, self.near, self.far)
        }
        else
        {
            perspective( self.window_dimensions.0 as f32 / self.window_dimensions.1 as f32,
                         nalgebra_glm::radians(&nalgebra_glm::vec1(self.fov))[0],
                         self.near_draw_distance, self.far_draw_distance)
        };

        let view_matrix = look_at(&self.position, &(&self.position + &self.direction), &self.up);

        Camera
        {
            direction: self.direction,
            position: self.position,
            projection_matrix,
            view_matrix,
            up: self.up,

            first_rotation: true,
            last_rotation_x: 0,
            last_rotation_y: 0,
            max_angle_look_up: self.max_angle_look_up,
            max_angle_look_down: self.max_angle_look_down,
            pitch: self.pitch,
            yaw: self.yaw,
            near_draw_distance: self.near_draw_distance,
            far_draw_distance: self.far_draw_distance,

            mouse_sensitivity: self.mouse_sensitivity,
            movement_speed_factor: self.movement_speed_factor,

            undefined_state: false,
            fov: self.fov,
            window_width: self.window_dimensions.0,
            window_height: self.window_dimensions.1,

            view_matrix_changed: false,
            draw_param_changed: false,
            window_dimensions_change: false,
        }
    }

    /// Update the value used for the created Camera's position
    ///
    /// `position` - the position the camera should have
    #[allow(dead_code)]
    pub fn with_position(&mut self, position: TVec3<f32>) -> &mut Self
    {
        self.position = position;
        self
    }

    /// Update the value used for the created Camera's direction. The given direction is normalized
    ///
    /// `direction` - the direction the camera should be facing
    #[allow(dead_code)]
    pub fn with_direction(&mut self, direction: TVec3<f32>) -> &mut Self
    {
        self.direction = nalgebra_glm::normalize(&direction);
        self
    }

    /// Update the value used for the created Camera's FOV
    ///
    /// `fov` - the field of view in degrees
    #[allow(dead_code)]
    pub fn with_fov(&mut self, fov: f32) -> &mut Self
    {
        self.fov = fov;
        self
    }

    /// Update the value used for the created Camera's near draw distance
    ///
    /// `near_draw_distance` - the near draw distance of the camera to create in world units
    #[allow(dead_code)]
    pub fn with_near_draw_distance(&mut self, near_draw_distance: f32) -> &mut Self
    {
        self.near_draw_distance = near_draw_distance;
        self
    }

    /// Update the value used for the created Camera's far draw distance
    ///
    /// `far_draw_distance` - the far draw distance of the camera to create in world units
    #[allow(dead_code)]
    pub fn with_far_draw_distance(&mut self, far_draw_distance: f32) -> &mut Self
    {
        self.far_draw_distance = far_draw_distance;
        self
    }

    /// Update the value used for the created Camera's mouse sensitivity, which changes how much
    /// movement it takes to rotate the camera
    ///
    /// `mouse_sensitivity` - the sensitivity of the mouse. Anything below one is less sensitive; anything
    ///                     more than one is greater sensitivity
    #[allow(dead_code)]
    pub fn with_mouse_sensitivity(&mut self, mouse_sensitivity: f32) -> &mut Self
    {
        self.mouse_sensitivity = mouse_sensitivity;
        self
    }

    /// Update the value used for the created Camera's speed factor. This changes how much the camera
    /// moves when pressing a movement key
    ///
    /// `speed_factor` - adjustment factor for the camera movement speed. Anything greater than one
    ///                  results in the camera moving faster; anything below one slows down the camera
    #[allow(dead_code)]
    pub fn with_movement_speed_factor(&mut self, speed_factor: f32) -> &mut Self
    {
        self.movement_speed_factor = speed_factor;
        self
    }

    /// Update the value used for the created Camera's pitch
    ///
    /// `pitch` - the initial pitch of the camera in degrees
    #[allow(dead_code)]
    pub fn with_pitch(&mut self, pitch: f32) -> &mut Self
    {
        self.pitch = pitch;
        self
    }

    /// Update the value used for the created Camera's yaw
    ///
    /// `yaw` - the initial yaw of the camera in degrees
    #[allow(dead_code)]
    pub fn with_yaw(&mut self, yaw: f32) -> &mut Self
    {
        self.yaw = yaw;
        self
    }

    /// Update the value used for the created Camera's max angle lookup
    ///
    /// `max_lookup` - the maximum pitch above 0 the camera can have (measurements are in degrees).
    ///                 Typical value is 89
    #[allow(dead_code)]
    pub fn with_max_lookup(&mut self, max_lookup: f32) -> &mut Self
    {
        self.max_angle_look_up = max_lookup;
        self
    }

    /// Update the value used for the created Camera's angle lookup
    ///
    /// `min_lookdown` - the minimum pitch below 0 the camera can have (measurements are in degrees).
    ///                 Typical value is -89
    #[allow(dead_code)]
    pub fn with_max_lookdown(&mut self, min_lookdown: f32) -> &mut Self
    {
        self.max_angle_look_down = min_lookdown;
        self
    }

    /// Sets the up vector of the camera to be built
    ///
    /// `up` - the up vector of the camera. Typically (0.0, 1.0, 0.0)
    #[allow(dead_code)]
    pub fn with_up_vector(&mut self, up: TVec3<f32>) -> &mut Self
    {
        self.up = up;
        self
    }

    /// Marks the created camera to be an orthographic camera
    #[allow(dead_code)]
    pub fn as_orthographic(&mut self) -> &mut Self
    {
        self.is_orthographic = true;
        self
    }

    /// Sets the bottom threshold of the orthographic camera
    ///
    /// `bottom` - bottom value of the created camera
    #[allow(dead_code)]
    pub fn with_bottom_ortho(&mut self, bottom: f32) -> &mut Self
    {
        self.bottom = bottom;
        self
    }

    /// Sets the top threshold of the orthographic camera
    ///
    /// `top` - top value of the created camera
    #[allow(dead_code)]
    pub fn with_top_ortho(&mut self, top: f32) -> &mut Self
    {
        self.top = top;
        self
    }

    /// Sets the left threshold of the orthographic camera
    ///
    /// `left` - left value of the created camera
    #[allow(dead_code)]
    pub fn with_left_ortho(&mut self, left: f32) -> &mut Self
    {
        self.left = left;
        self
    }

    /// Sets the right threshold of the orthographic camera
    ///
    /// `right` - right value of the created camera
    #[allow(dead_code)]
    pub fn with_right_ortho(&mut self, right: f32) -> &mut Self
    {
        self.right = right;
        self
    }

    /// Sets the far threshold of the orthographic camera. This is the far draw distance
    ///
    /// `far` - far value of the created camera
    #[allow(dead_code)]
    pub fn with_far_ortho(&mut self, far: f32) -> &mut Self
    {
        self.far = far;
        self
    }

    /// Sets the far threshold of the orthographic camera. This is the near draw distance
    ///
    /// `near` - near value of the created camera
    #[allow(dead_code)]
    pub fn with_near_ortho(&mut self, near: f32) -> &mut Self
    {
        self.near = near;
        self
    }
}