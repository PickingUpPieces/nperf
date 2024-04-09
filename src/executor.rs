use log::{debug, error, info};

use crate::command::nPerf;
use crate::net::socket::Socket;
use crate::node::{client::Client, server::Server, Node};
use crate::util::core_affinity_manager::CoreAffinityManager;
use crate::util::{statistic::{MultiplexPort, Parameter, SimulateConnection}, NPerfMode};
use crate::{Statistic, DEFAULT_CLIENT_IP};

use std::sync::{Arc, Mutex};
use std::{cmp::max, net::SocketAddrV4, sync::mpsc::{self, Sender}, thread};
extern crate core_affinity;

impl nPerf {
    pub fn exec(self, parameter: Parameter) -> Option<Statistic> {
        info!("Starting nPerf...");
        debug!("Running with Parameter: {:?}", parameter);

        let core_affinity_manager = Arc::new(Mutex::new(CoreAffinityManager::new(parameter.mode == NPerfMode::Server)));
        if parameter.core_affinity {
            core_affinity_manager.lock().unwrap().set_affinity().expect("Error setting core affinity");
        }

        loop {
            let mut fetch_handle: Vec<thread::JoinHandle<()>> = Vec::new();
            let (tx, rx) = mpsc::channel();
    
            // If socket sharing enabled, creating the socket and bind to port/connect must happen before the threads are spawned
            let socket = self.create_socket(parameter);


            for i in 0..parameter.amount_threads {
                let tx: Sender<_> = tx.clone();

                let server_port = if parameter.multiplex_port_server != MultiplexPort::Individual {
                    info!("Server port is shared/sharded. Incrementing port number is disabled.");
                    self.port
                } else {
                    self.port + i
                };

                // Get instance of core affinity manager
                let core_affinity = Arc::clone(&core_affinity_manager);
                // Use same test id for all threads if one connection is simulated
                let test_id = if parameter.simulate_connection == SimulateConnection::Single { 0 } else { i as u64 };
                let local_port_client: Option<u16> = if parameter.multiplex_port == MultiplexPort::Sharding { Some(self.client_port) } else { None };

                fetch_handle.push(thread::spawn(move || Self::exec_thread(parameter, tx, socket, server_port, local_port_client, test_id, core_affinity)));
            }
    
            info!("Waiting for all threads to finish...");
            let mut statistics = Self::get_statistics(fetch_handle, rx, parameter);

            info!("All threads finished!");
            if let Some(socket) = socket {
                socket.close().expect("Error closing socket");
            }
    
            if statistics.amount_datagrams != 0 {
                statistics.calculate_statistics();
                statistics.print(parameter.output_format);
            }

            if !(self.run_infinite && parameter.mode == NPerfMode::Server) {
                return Some(statistics);
            }
        }
    }

    fn exec_thread(parameter: Parameter, tx: mpsc::Sender<Option<Statistic>>, socket: Option<Socket>, server_port: u16, client_port: Option<u16>, test_id: u64, core_affinity_manager: Arc<Mutex<CoreAffinityManager>>) {
        let sock_address_server = SocketAddrV4::new(parameter.ip, server_port);

        if parameter.core_affinity {
            core_affinity_manager.lock().unwrap().set_affinity().unwrap();
        }
        
        let mut node:Box<dyn Node> = if parameter.mode == NPerfMode::Client {
            Box::new(Client::new(test_id, client_port, sock_address_server, socket, parameter))
        } else {
            Box::new(Server::new(sock_address_server, socket, parameter))
        };

        match node.run(parameter.io_model) {
            Ok(statistic) => { 
                info!("Finished measurement!");
                tx.send(Some(statistic)).unwrap();
            },
            Err(x) => {
                error!("Error running app: {}", x);
                tx.send(None).unwrap();
            }
        }
    }

    fn get_statistics(fetch_handle: Vec<thread::JoinHandle<()>>, rx: mpsc::Receiver<Option<Statistic>>, parameter: Parameter) -> Statistic {
        fetch_handle.into_iter().fold(Statistic::new(parameter), |acc: Statistic, handle| { 
            let statistic = acc + match rx.recv_timeout(std::time::Duration::from_secs(max(parameter.test_runtime_length * 2, 120))) {
                Ok(x) => {
                    match x {
                    Some(x) => x,
                    None => Statistic::new(parameter)
                    }
                },
                Err(_) => Statistic::new(parameter)
            };
                
            handle.join().unwrap(); 
            statistic 
        })
    }

    fn create_socket(&self, parameter: Parameter) -> Option<Socket> {
        if parameter.mode == NPerfMode::Client && parameter.multiplex_port == MultiplexPort::Sharing {
            let mut socket = Socket::new(parameter.socket_options).expect("Error creating socket");
            let sock_address_in = SocketAddrV4::new(DEFAULT_CLIENT_IP, self.client_port);

            socket.bind(sock_address_in).expect("Error binding to local port");

            // connect (includes bind) to specific 4-tuple, since every thread sends to same port on the server side
            if parameter.multiplex_port_server == MultiplexPort::Sharding || parameter.multiplex_port_server == MultiplexPort::Sharing {
                let sock_address_out = SocketAddrV4::new(parameter.ip, self.port);
                socket.connect(sock_address_out).expect("Error connecting to remote host");
            }

            Some(socket)
        } else if parameter.mode == NPerfMode::Server && parameter.multiplex_port_server == MultiplexPort::Sharing {
            let sock_address_in = SocketAddrV4::new(parameter.ip, self.port);
            let mut socket = Socket::new(parameter.socket_options).expect("Error creating socket");
            socket.bind(sock_address_in).expect("Error binding to local port");
            Some(socket)
        } else {
            None
        }
    }
}
