use std::net::Ipv4Addr;
use std::time::Instant;

use libc::close;
use log::{debug, error, info};

use crate::util;
use crate::net;
use crate::util::History;

struct Client {
    ip: Ipv4Addr,
    remote_port: u16,
    mtu_size: usize,
    mtu_discovery: bool,
    buffer: Vec<u8>,
    socket: i32,
    history: History,
    run_time_length: u64,
}


impl Client {
    pub fn new(ip: Ipv4Addr, remote_port: u16, mtu_size: usize, mtu_discovery: bool, run_time_length: u64) -> Client {
        let socket = net::create_socket().expect("Error creating socket"); 

        Client {
            ip,
            remote_port,
            mtu_size,
            mtu_discovery,
            buffer: vec![0; mtu_size],
            socket,
            history: History::new(mtu_size as u64),
            run_time_length,
        }
    }

    pub fn run(&self) {
        info!("Current mode: client");
        // Fill the buffer
        util::fill_buffer_with_repeating_pattern(&mut self.buffer);
    
        net::set_socket_send_buffer_size(self.socket, crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE).expect("Error setting socket send buffer size");
    
        net::connect(self.socket, self.ip, self.remote_port).expect("Error connecting to remote host"); 
    
        net::set_socket_nonblocking(self.socket).expect(msg);
    
        if self.mtu_discovery {
            self.buffer = util::create_buffer_dynamic(self.socket);
        }
    
        self.history.start_time = Instant::now();
        let buffer_length = self.buffer.len();
    
        while self.history.start_time.elapsed().as_secs() < self.run_time_length {
            util::prepare_packet(self.history.amount_datagrams, &mut self.buffer);
    
            match net::send(self.socket, &mut self.buffer, buffer_length) {
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
        match net::send(self.socket, &mut last_message_buffer, crate::LAST_MESSAGE_SIZE as usize) {
            Ok(_) => {
                self.history.end_time = Instant::now();
                debug!("Finished sending data to remote host");
                self.history.print();
            },
            Err(x) => { 
                error!("{x}"); 
                panic!()},
        };
    
        // Close file descriptor at last
        unsafe { close(measurement.socket) }; 
    }
}