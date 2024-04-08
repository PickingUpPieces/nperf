// DISCLAIMER
// Code was partly taken from https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/utils/cpu_affinity.rs
use std::thread;
use log::info;

pub struct CoreAffinityManager {
    core_ids: Vec<core_affinity::CoreId>,
    last_core_id: usize,
    inverse_round_robin: bool, // Server uses inverse round robin
}

impl CoreAffinityManager {
    pub fn new(inverse_round_robin: bool) -> CoreAffinityManager {
        let core_ids = core_affinity::get_core_ids().unwrap_or_default();
        CoreAffinityManager {
            core_ids,
            last_core_id: 0,
            inverse_round_robin,
        }
    }

    pub fn set_affinity(&mut self) -> Result<(), &'static str> {
        let core_id = self.get_core_id()?;

        if core_affinity::set_for_current(core_id) == true {
            info!("{:?}: Setting CPU affinity to core number {} (of {} total cores)", thread::current().id(), core_id.id, self.core_ids.len());
            Ok(())
        } else {
            Err("Error setting CPU affinity!")
        }
    }
    
    pub fn get_core_id(&mut self) -> Result<core_affinity::CoreId, &'static str> {
        if self.core_ids.len() <= 0 {
            return Err("No core IDs available! CPU affinity is not configured correctly.");
        }

        //cycle to the next core ID in a (reverse) round-robin order
        if self.inverse_round_robin {
            if self.last_core_id == 0 {
                self.last_core_id = self.core_ids.len() - 1;
            } else {
                self.last_core_id -= 1;
            }
        } else {
            if self.last_core_id == self.core_ids.len() - 1 {
                self.last_core_id = 0;
            } else {
                self.last_core_id += 1;
            }
        }

        Ok(self.core_ids[self.last_core_id]) 
    }
}
