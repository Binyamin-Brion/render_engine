use lazy_static::lazy_static;
use std::collections::HashMap;
use render_engine::objects::entity_id::EntityId;
use std::sync::Mutex;

lazy_static!
{
     pub static ref INSTANCES: Mutex<HashMap<String, InstanceInfo>> = Mutex::new(HashMap::new());
     static ref INSTANCE_COUNT: Mutex<u32> = Mutex::new(0);
}

pub struct InstanceInfo
{
    num_instances: usize,
    pub specific_instance: HashMap<String, EntityId>
}

pub fn generate_random_name() -> String
{
    let mut guard = INSTANCE_COUNT.lock().unwrap();

    let return_result = guard.to_string();
    *guard += 1;

    return_result
}

impl InstanceInfo
{
    pub fn new(num_instances: usize) -> InstanceInfo
    {
        InstanceInfo { num_instances, specific_instance: HashMap::new() }
    }

    pub fn get_num_instances(&self) -> usize
    {
        self.num_instances
    }
}