use std::net::Ipv4Addr;
use std::time::Instant;
use log::trace;
use log::{debug, error, info};

use crate::util;
use crate::net::socket::Socket;
use crate::util::History;

pub struct Client {
    mtu_discovery: bool,
    buffer: Vec<u8>,
    socket: Socket,
    history: History,
    run_time_length: u64,
}


impl Client {
    pub fn new(ip: Ipv4Addr, remote_port: u16, mtu_size: usize, mtu_discovery: bool, run_time_length: u64) -> Client {
        let socket = Socket::new(ip, remote_port, mtu_size).expect("Error creating socket");

        Client {
            mtu_discovery,
            buffer: vec![0; mtu_size],
            socket,
            history: History::new(mtu_size as u64),
            run_time_length,
        }
    }

    pub fn run(&mut self) {
        info!("Current mode: client");
        util::fill_buffer_with_repeating_pattern(&mut self.buffer);
        self.socket.set_send_buffer_size(crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE).expect("Error setting socket send buffer size");
        self.socket.connect().expect("Error connecting to remote host"); 
        self.socket.set_nonblocking().expect("Error setting socket to nonblocking mode");
    
        if self.mtu_discovery {
            self.buffer = util::create_buffer_dynamic(&mut self.socket);
        }
    
        self.history.start_time = Instant::now();
        let buffer_length = self.buffer.len();
    
        while self.history.start_time.elapsed().as_secs() < self.run_time_length {
            util::prepare_packet(self.history.amount_datagrams, &mut self.buffer);
    
            match self.socket.send(&mut self.buffer, buffer_length) {
                Ok(_) => {
                    self.history.amount_datagrams += 1;
                    trace!("Sent datagram to remote host");
                },
                Err("ECONNREFUSED") => {
                    error!("Start the server first! Abort measurement...");
                    return;
                },
                Err(x) => { 
                    error!("{x}"); 
                    panic!()},
            };
        }
    
        let mut last_message_buffer: [u8; crate::LAST_MESSAGE_SIZE as usize] = [0; crate::LAST_MESSAGE_SIZE as usize];

        // TODO: Unwrap and do something if it's successfull
        match self.socket.send(&mut last_message_buffer, crate::LAST_MESSAGE_SIZE as usize) {
            Ok(_) => {
                self.history.end_time = Instant::now();
                debug!("Finished sending data to remote host");
                self.history.print();
            },
            Err(x) => { 
                error!("{x}"); 
                panic!()},
        };
    }
}
