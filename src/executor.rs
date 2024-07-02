use log::{debug, error, info, warn};

use crate::command::nPerf;
use crate::io_uring::normal::IoUringNormal;
use crate::io_uring::IoUringOperatingModes;
use crate::net::socket::Socket;
use crate::node::{sender::Sender, receiver::Receiver, Node};
use crate::util::core_affinity_manager::CoreAffinityManager;
use crate::util::{statistic::{MultiplexPort, Parameter, SimulateConnection}, NPerfMode};
use crate::Statistic;

use std::os::fd::RawFd;
use std::sync::{Arc, Mutex};
use std::{net::SocketAddrV4, thread};
extern crate core_affinity;

impl nPerf {
    pub fn exec(self, parameter: Parameter) -> Option<Statistic> {
        info!("Starting nPerf...");
        debug!("Running with Parameter: {:?}", parameter);

        let core_affinity_manager = Arc::new(Mutex::new(CoreAffinityManager::new(parameter.mode, None, parameter.numa_affinity)));

        if parameter.core_affinity {
            core_affinity_manager.lock().unwrap().bind_to_core(0).expect("Error setting affinity");
        }

        loop {
            #[allow(clippy::type_complexity)]
            let mut fetch_handle: Vec<thread::JoinHandle<Result<(Statistic, Vec<Statistic>), &str>>> = Vec::new();
    
            // If socket sharing enabled, creating the socket and bind to port/connect must happen before the threads are spawned
            let socket = self.create_socket(&parameter);

            // If SQ_POLL and io_uring enabled, create io_uring fd here
            let io_uring: Option<IoUringNormal> = if parameter.uring_parameter.sqpoll_shared {
                IoUringNormal::new(parameter.clone(), None).ok()
            } else {
                None
            };
            let io_uring_fd = io_uring.as_ref().map(|io_uring| io_uring.get_raw_fd()); 


            for i in 0..parameter.amount_threads {
                let receiver_port = if parameter.multiplex_port_receiver != MultiplexPort::Individual {
                    info!("Receiver port is shared/sharded. Incrementing port number is disabled.");
                    self.port
                } else {
                    self.port + i
                };

                // Get instance of core affinity manager
                let core_affinity = Arc::clone(&core_affinity_manager);
                // Use same test id for all threads if one connection is simulated
                let test_id = if parameter.simulate_connection == SimulateConnection::Single { 0 } else { i as u64 };
                let local_port_sender: Option<u16> = if parameter.multiplex_port == MultiplexPort::Sharding { Some(self.sender_port) } else { None };
                let parameter_clone = parameter.clone();

                fetch_handle.push(thread::spawn(move || Self::exec_thread(parameter_clone, socket, io_uring_fd, receiver_port, local_port_sender, test_id, core_affinity)));
            }
    
            info!("Waiting for all threads to finish...");
            let mut util = crate::util::cpu_util::CpuUtil::new();
            util.get_relative_cpu_util();

            let amount_interval_outputs = if parameter.output_interval == 0.0 { 0 } else { (parameter.test_runtime_length as f64 / parameter.output_interval).floor() as usize };
            debug!("Amount of interval outputs: {}", amount_interval_outputs);


            // Iter over join handle and sum up statistics
            let mut final_statistics = Statistic::new(parameter.clone());
            let mut final_interval_statistics: Vec<Statistic> = vec![Statistic::new(parameter.clone()); amount_interval_outputs];
            for (interval_id, statistic) in final_interval_statistics.iter_mut().enumerate() {
                statistic.interval_id = interval_id as u64 + 1;
            }

            for handle in fetch_handle {
                match handle.join() {
                    Ok(result) => {
                        // TODO: Merge interval_statistics
                        if let Ok((statistic, interval_statistics)) = result { 
                            final_statistics = final_statistics + statistic;
                            if amount_interval_outputs != 0 {
                                for statistic in interval_statistics {
                                    let interval_id = statistic.interval_id as usize - 1;
                                    final_interval_statistics[interval_id] = final_interval_statistics[interval_id].clone() + statistic;
                                }
                            } 
                        }
                    },
                    Err(x) => warn!("Error joining thread: {:?}", x),
                }
            }

            for statistic in final_interval_statistics.iter_mut() {
                // Fix interval CPU util: (statistics.cpu_user_time, statistics.cpu_system_time, statistics.cpu_total_time) = util.get_relative_cpu_util();
                if statistic.amount_datagrams != 0 {
                    statistic.print(parameter.output_format, true);
                }
            }

            // Update CPU spent time
            (final_statistics.cpu_user_time, final_statistics.cpu_system_time, final_statistics.cpu_total_time) = util.get_absolut_cpu_util();

            if final_statistics.amount_datagrams != 0 {
                final_statistics.print(parameter.output_format, false);
            }

            info!("All threads finished!");
            if let Some(socket) = socket {
                socket.close().expect("Error closing socket");
            }
    
            if !(self.run_infinite && parameter.mode == NPerfMode::Receiver) {
                return Some(final_statistics);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_thread(parameter: Parameter, socket: Option<Socket>, io_uring: Option<RawFd>, receiver_port: u16, sender_port: Option<u16>, test_id: u64, core_affinity_manager: Arc<Mutex<CoreAffinityManager>>) -> Result<(Statistic, Vec<Statistic>), &'static str> {
        let sock_address_receiver = SocketAddrV4::new(parameter.ip, receiver_port);

        if parameter.core_affinity {
            core_affinity_manager.lock().unwrap().set_affinity().unwrap();
        }
        
        let mut node: Box<dyn Node> = if parameter.mode == NPerfMode::Sender {
            Box::new(Sender::new(test_id, sender_port, sock_address_receiver, socket, io_uring, parameter.clone()))
        } else {
            Box::new(Receiver::new(sock_address_receiver, socket, io_uring, parameter.clone()))
        };

        match node.run(parameter.io_model) {
            Ok(statistic) => { 
                info!("{:?}: Finished measurement!", thread::current().id());
                Ok(statistic)
            },
            Err(x) => {
                error!("{:?}: Error running app: {}", thread::current().id(), x);
                Err("Error running app")
            }
        }
    }


    fn create_socket(&self, parameter: &Parameter) -> Option<Socket> {
        if parameter.mode == NPerfMode::Sender && parameter.multiplex_port == MultiplexPort::Sharing {
            info!("Creating master socket for all sender threads to use, since socket sharing is enabled");
            let mut socket = Socket::new(parameter.socket_options).expect("Error creating socket");
            let sock_address_in = SocketAddrV4::new(crate::DEFAULT_SENDER_IP, self.sender_port);

            socket.bind(sock_address_in).expect("Error binding to local port");

            // connect (includes bind) to specific 4-tuple, since every thread sends to same port on the receiver side
            if parameter.multiplex_port_receiver == MultiplexPort::Sharding || parameter.multiplex_port_receiver == MultiplexPort::Sharing {
                let sock_address_out = SocketAddrV4::new(parameter.ip, self.port);
                socket.connect(sock_address_out).expect("Error connecting to remote host");
            }

            Some(socket)
        } else if parameter.mode == NPerfMode::Receiver && parameter.multiplex_port_receiver == MultiplexPort::Sharing {
            info!("Creating master socket for all receiver threads to use, since socket sharing is enabled");
            let sock_address_in = SocketAddrV4::new(parameter.ip, self.port);
            let mut socket = Socket::new(parameter.socket_options).expect("Error creating socket");
            socket.bind(sock_address_in).expect("Error binding to local port");
            Some(socket)
        } else {
            None
        }
    }
}
