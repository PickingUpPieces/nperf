
use std::net::Ipv4Addr;
use std::time::Instant;

use libc::close;
use log::{debug, error, info};

use crate::util::*;
use crate::net;


struct Server {
    ip: Ipv4Addr,
    local_port: u16,
    mtu_size: usize,
    mtu_discovery: bool,
    buffer: Vec<u8>,
    socket: i32,
    run_infinite: bool,
    first_packet_received: bool,
    next_packet_id: u64,
    history: History,
}

impl Server {
    pub fn new(ip: Ipv4Addr, local_port: u16, mtu_size: usize, mtu_discovery: bool, run_infinite: bool) -> Server {
        let socket = net::create_socket().expect("Error creating socket"); 

        Server {
            ip,
            local_port,
            mtu_size,
            mtu_discovery,
            buffer: vec![0; mtu_size],
            socket,
            run_infinite,
            first_packet_received: false,
            next_packet_id: 0,
            history: History::new(mtu_size as u64),
        }
    }

    pub fn run(&self) {
        info!("Current mode: server");
        net::bind_socket(self.socket, self.ip, self.local_port).expect("Error binding socket");

        net::set_socket_receive_buffer_size(self.socket, crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE).expect("Error setting socket receive buffer size"); 

        net::set_socket_nonblocking(self.socket).expect("Error setting socket to nonblocking mode");

        loop {
            match net::recv(self.socket, &mut self.buffer) {
                Ok(amount_received_bytes) => {
                    if self.first_packet_received == false {
                        self.first_packet_received = true;
                        info!("First packet received!");

                        if self.mtu_discovery {
                            // FIXME: getting the IP_MTU from getsockopt throws an error, therefore don't use it for now
                            info!("Set buffer size to MTU");
                            self.buffer = util::create_buffer_dynamic(self.socket);
                        }

                        self.history.start_time = Instant::now();
                    }

                    if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                        info!("Last packet received!");
                        break;
                    }
                    self.next_packet_id += util::process_packet(&mut self.buffer, self.next_packet_id, &mut self.history);
                    self.history.amount_datagrams += 1;
                },
                Err("EAGAIN") => continue,
                Err(x) => {
                    error!("{x}"); 
                    unsafe { close(self.socket) }; 
                    panic!();
                }
            };
        }

        self.history.end_time = Instant::now();
        debug!("Finished receiving data from remote host");
        self.history.print();
    }
}