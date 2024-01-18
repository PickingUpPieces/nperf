
use std::net::Ipv4Addr;
use std::time::Instant;
use log::{debug, error, info};

use crate::util;
use crate::net::socket::Socket;
use crate::util::History;


pub struct Server {
    mtu_discovery: bool,
    buffer: Vec<u8>,
    socket: Socket,
    _run_infinite: bool,
    first_packet_received: bool,
    next_packet_id: u64,
    history: History,
}

impl Server {
    pub fn new(ip: Ipv4Addr, local_port: u16, mtu_size: usize, mtu_discovery: bool, use_gso: bool, run_infinite: bool) -> Server {
        let socket = Socket::new(ip, local_port, mtu_size, use_gso).expect("Error creating socket");

        Server {
            mtu_discovery,
            buffer: vec![0; mtu_size],
            socket,
            _run_infinite: run_infinite,
            first_packet_received: false,
            next_packet_id: 0,
            history: History::new(mtu_size as u64),
        }
    }

    pub fn run(&mut self) {
        info!("Current mode: server");
        self.socket.bind().expect("Error binding socket");
        self.socket.set_receive_buffer_size(crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE).expect("Error setting socket receive buffer size"); 
        self.socket.set_nonblocking().expect("Error setting socket to nonblocking mode");

        loop {
            match self.socket.read(&mut self.buffer) {
                Ok(amount_received_bytes) => {
                    if self.first_packet_received == false {
                        self.first_packet_received = true;
                        info!("First packet received!");

                        if self.mtu_discovery {
                            // FIXME: getting the IP_MTU from getsockopt throws an error, therefore don't use it for now
                            info!("Set buffer size to MTU");
                            self.buffer = util::create_buffer_dynamic(&mut self.socket);
                            self.history.datagram_size = self.buffer.len() as u64;
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
                    panic!();
                }
            };
        }

        self.history.end_time = Instant::now();
        debug!("Finished receiving data from remote host");
        self.history.print();
    }
}