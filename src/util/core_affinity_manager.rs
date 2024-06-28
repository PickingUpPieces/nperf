// DISCLAIMER
// Code was partly taken from https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/utils/cpu_affinity.rs
use std::thread;
use log::info;

use super::NPerfMode;

pub struct CoreAffinityManager {
    core_ids: Vec<core_affinity::CoreId>,
    next_core_id: i32,
    mode: NPerfMode, // Server uses inverse round robin
    numa_affinity: bool // According to the NUMA core pattern on the server, assign the threads to run on the same NUMA node
}

impl CoreAffinityManager {
    pub fn new(mode: NPerfMode, first_core_id: Option<i32>, numa_affinity: bool) -> CoreAffinityManager {
        let core_ids = core_affinity::get_core_ids().unwrap_or_default();

        let next_core_id = if let Some(next_core_id) = first_core_id {
            next_core_id
        } else if numa_affinity {
            match mode {
                NPerfMode::Client => 1,
                NPerfMode::Server => (core_ids.len() - 1 ) as i32
            }
        } else {
            match mode {
                NPerfMode::Client => 1,
                NPerfMode::Server => 2
            }
        };

        CoreAffinityManager {
            core_ids,
            next_core_id,
            mode,
            numa_affinity
        }
    }

    pub fn set_affinity(&mut self) -> Result<(), &'static str> {
        let core_id = self.get_core_id()?;

        if core_affinity::set_for_current(core_id) {
            info!("{:?}: Setting CPU affinity to core number {} (of {} total cores)", thread::current().id(), core_id.id, self.core_ids.len());
            Ok(())
        } else {
            Err("Error setting CPU affinity!")
        }
    }
    
    // We don't schedule any thread on core 0, since sometimes interrupts are scheduled on core 0
    fn get_core_id(&mut self) -> Result<core_affinity::CoreId, &'static str> {
        if self.core_ids.is_empty() {
            return Err("No core IDs available! CPU affinity is not configured correctly.");
        }

        let ret = self.next_core_id;

        // If NUMA enabled, schedule client/server pair of threads both on uneven on even/uneven core_ids only
        let delta = if self.numa_affinity {
            1
        } else {
            2
        };

        // cycle to the next core ID 
        self.next_core_id = {
            if self.numa_affinity && self.mode == NPerfMode::Server {
                // If we assign NUMA, the reverse round-robin schedules the uneven core IDs
                if self.next_core_id - delta <= 0 {
                    ( self.core_ids.len() - 1 ) as i32
                } else {
                    self.next_core_id - delta
                }
            } else if self.numa_affinity && self.mode == NPerfMode::Client {
                if self.next_core_id + delta >= self.core_ids.len() as i32 {
                    1
                } else {
                    self.next_core_id + delta
                }
            // numa_affinity is not enabled
            } else if self.next_core_id + delta >= self.core_ids.len() as i32 {
                match self.mode {
                    // When we wrap around the core_ids, we use now core 0 as well
                    // We assume an even number of cores, therefore if we start with core 1, the server is the first one to wrap around
                    NPerfMode::Client => 1,
                    NPerfMode::Server => 0
                }
            } else {
                self.next_core_id + delta
            }
        };

        Ok(self.core_ids[ret as usize]) 
    }
}
