/// Macro that handles any generic input keys. This is more generic than the macro that handles
/// camera movement keys
#[macro_export]
macro_rules! handle_key_input
{
    ($window: ident, $key: ident, $action: ident, $logic: expr) =>
    {
        let right_key_pressed = |x: (Key, Action)| x.0 == Key::$key && x.1 == Action::$action;
        if $window.check_keys(right_key_pressed)
        {
            $logic
        }
    };

    ($window: ident, $key: ident, $action: ident, $logic: block) =>
    {
        let right_key_pressed = |x: (Key, Action)| x.0 == Key::$key && x.1 == Action::$action;
        if $window.check_keys(right_key_pressed)
        {
            $logic
        }
    };
}

/// Macro that deals only with movement keys of the camera
#[macro_export]
macro_rules! handle_wasd_movement
{
    ($window: ident, $camera: ident, $delta_time: ident) =>
    {
        $camera.write().use_wasd_keys($window.get_movement_keys(), $delta_time);
    };
}

/// Macro to deal with changes in the cursor position
#[macro_export]
macro_rules! handle_cursor_movement
{
    ($window: ident, $camera: ident) =>
    {
        if $window.middle_button_down()
        {
            if let Some((new_x, new_y)) = $window.get_latest_cursor_pos()
            {
                $camera.write().rotate(new_x, new_y);
            }
        }
        else
        {
            $camera.write().first_rotation = true;
        }
    };
}