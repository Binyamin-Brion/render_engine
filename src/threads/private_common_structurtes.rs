use std::sync::Arc;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use crate::exports::camera_object::Camera;

// These variables are shared only between the history and render thread modules
lazy_static!
{
    pub static ref CAMERA: Arc<RwLock<Camera>> = Arc::new(RwLock::new(Camera::new_undefined()));
    pub static ref DELTA_TIME: Arc<RwLock<f32>> = Arc::new(RwLock::new(0.0));
}
