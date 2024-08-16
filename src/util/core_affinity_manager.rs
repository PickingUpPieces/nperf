use std::thread;

use hwlocality::{cpu::{binding::CpuBindingFlags, cpuset::CpuSet}, object::types::ObjectType, topology::support::{DiscoverySupport, FeatureSupport}, Topology};
use log::{info, warn};

use super::NPerfMode;

pub struct CoreAffinityManager {
    topology: hwlocality::Topology,
    mode: NPerfMode,
    numa_affinity: bool,
    amount_cpus: usize,
    next_numa_node: u64,
    next_core_id: usize 
}

impl CoreAffinityManager {
    pub fn new(mode: NPerfMode, first_core_id: Option<usize>, mut numa_affinity: bool) -> CoreAffinityManager {
        let topology = Topology::new().unwrap();

        if !topology.supports(FeatureSupport::discovery, DiscoverySupport::pu_count) {
            panic!("If core-affinity enabled, nPerf needs accurate reporting of PU objects");
        }
        let Some(cpu_support) = topology.feature_support().cpu_binding() else {
            panic!("If core-affinity enabled, nPerf requires CPU binding support");
        };
        if !(cpu_support.get_thread() && cpu_support.set_thread()) {
            panic!("If core-affinity enabled, nPerf needs support for querying and setting thread CPU bindings");
        }

        let mut amount_cpus = topology.objects_with_type(ObjectType::PU).count();

        if numa_affinity {
            let numa_nodes = topology.objects_with_type(ObjectType::NUMANode).collect::<Vec<_>>();
            if numa_nodes.is_empty() {
                warn!("NUMA is not available on this system! Disabling NUMA affinity.");
                numa_affinity = false;
            } else {
                let cpuset = numa_nodes.first().unwrap().cpuset().unwrap();
                let num_cores = cpuset.weight();
                amount_cpus = num_cores.unwrap();
                info!("NUMA is available with {} nodes and {} CPUs per NUMA node ", amount_cpus, numa_nodes.len());
            }
        } 

        let next_core_id = match mode {
            NPerfMode::Receiver => {
                first_core_id.unwrap_or(amount_cpus - 1)
            }, 
            NPerfMode::Sender => {
                first_core_id.unwrap_or(0)
            }
        };

        CoreAffinityManager {
            topology,
            mode,
            numa_affinity,
            amount_cpus,
            next_numa_node: 0,
            next_core_id
        }
    }

    pub fn set_affinity(&mut self) -> Result<(), &'static str> {
        let mut core_id = self.get_core_id();

        core_id = if self.numa_affinity {
            // Get the NUMA node to schedule thread on
            let numa_nodes = self.topology.objects_with_type(ObjectType::NUMANode).collect::<Vec<_>>();
            if self.next_numa_node as usize >= numa_nodes.len() {
                return Err("NUMA node index out of bounds");
            }
            // In NUMA, the returned core_id is relativ for the NUMA node. We need to get the absolut core ID
            let numa_node = &numa_nodes.get(self.next_numa_node as usize).unwrap();
            let core_bitmap = numa_node.cpuset().ok_or("Failed to get CPU set for NUMA node")?;
            match core_bitmap.iter_set().nth(core_id) {
                Some(absolute_core_id) => {
                usize::from(absolute_core_id) 
                },
                None => return Err("Failed to get core ID from CPU set")
            }
        } else {
            core_id
        };

        info!("Binding thread {:?} to core ID: {}", thread::current().id(), core_id);
        let mut core_cpuset = CpuSet::new();
        core_cpuset.set(core_id);
        self.bind_to_cpuset(core_cpuset)
    }

    fn get_core_id(&mut self) -> usize {
        let return_core_id = self.next_core_id;
        let delta: isize = if self.mode == NPerfMode::Receiver { -1 } else { 1 }; 

        if self.numa_affinity {
            if self.forward_numa_node() == 0 {
                // Only go to next relativ core, if we iterated over all numa nodes
                self.next_core_id = ((return_core_id as isize + delta) % self.amount_cpus as isize) as usize;
            }
        } else {
            self.next_core_id = ((return_core_id as isize + delta) % self.amount_cpus as isize) as usize;
        }

        return_core_id
    }

    // Go to the next NUMA node
    fn forward_numa_node(&mut self) -> u64 {
        let numa_nodes = self.topology.objects_with_type(ObjectType::NUMANode).collect::<Vec<_>>();
        self.next_numa_node = (self.next_numa_node + 1) % numa_nodes.len() as u64;
        self.next_numa_node
    }

    pub fn bind_to_core(&mut self, core_id: usize) -> Result<(), &'static str> {
        let mut core_cpuset = CpuSet::new();
        core_cpuset.set(core_id);
        self.bind_to_cpuset(core_cpuset)
    }

    fn bind_to_cpuset(&mut self, core_cpuset: CpuSet) -> Result<(), &'static str> {
        match self.topology.bind_cpu(&core_cpuset, CpuBindingFlags::THREAD) {
            Ok(_) => {
                Ok(())
            },
            Err(x) => {
                warn!("Error binding thread to core: {}", x);
                Err("Error binding thread to core")
            }
        }
    }
}
