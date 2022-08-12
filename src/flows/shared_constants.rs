use lazy_static::lazy_static;
use parking_lot::Mutex;

lazy_static!
{
  pub static ref WORLD_SECTION_LENGTH: Mutex<u32> = Mutex::new(32);
}