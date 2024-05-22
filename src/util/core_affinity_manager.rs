// DISCLAIMER
// Code was partly taken from https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/utils/cpu_affinity.rs
use std::thread;
use log::info;

pub struct CoreAffinityManager {
    core_ids: Vec<core_affinity::CoreId>,
    next_core_id: usize,
    inverse_round_robin: bool, // Server uses inverse round robin
    assign_numa: bool // According to the NUMA core pattern on the server, assign the threads to run on the same NUMA node
}

const NUMA_CORE_PATTERN: usize = 2;

impl CoreAffinityManager {
    pub fn new(inverse_round_robin: bool, first_core_id: Option<usize>, assign_numa: bool) -> CoreAffinityManager {
        let core_ids = core_affinity::get_core_ids().unwrap_or_default();

        let next_core_id = if let Some(next_core_id) = first_core_id {
            next_core_id
        } else {
            0
        };

        CoreAffinityManager {
            core_ids,
            next_core_id,
            inverse_round_robin,
            assign_numa
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
    
    fn get_core_id(&mut self) -> Result<core_affinity::CoreId, &'static str> {
        if self.core_ids.is_empty() {
            return Err("No core IDs available! CPU affinity is not configured correctly.");
        }

        let ret = self.next_core_id;

        // If NUMA enabled, schedule threads on even/uneven core_ids only
        let delta = if self.assign_numa {
            NUMA_CORE_PATTERN
        } else {
            1
        };

        // cycle to the next core ID 
        // Reverse round-robin order
        #[allow(clippy::collapsible_else_if)]
        if self.inverse_round_robin {
            // If we assign NUMA, the reverse round-robin schedules the uneven core IDs
            if self.next_core_id < delta {
                self.next_core_id = self.core_ids.len() - 1;
            } else {
                self.next_core_id -= delta;
            }
        } else {
            // normal round-robin
            // If we assign NUMA, the normal round-robin schedules the even core IDs
            if self.next_core_id == self.core_ids.len() - delta {
                self.next_core_id = 0;
            } else {
                self.next_core_id += delta;
            }
        }

        Ok(self.core_ids[ret]) 
    }
}
