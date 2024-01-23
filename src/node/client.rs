use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
use log::trace;
use log::{debug, error, info};

use crate::net::socket_options::SocketOptions;
use crate::util::{self, ExchangeFunction};
use crate::net::socket::Socket;
use crate::util::history::History;
use crate::util::packet_buffer::PacketBuffer;

use super::Node;

#[derive(Debug)]
pub struct Client {
    mtu_discovery: bool,
    packet_buffer: PacketBuffer,
    socket: Socket,
    history: History,
    run_time_length: u64,
    next_packet_id: u64,
    exchange_function: ExchangeFunction,
}


impl Client {
    pub fn new(ip: Ipv4Addr, remote_port: u16, mtu_size: usize, mtu_discovery: bool, socket_options: SocketOptions, run_time_length: u64, exchange_function: ExchangeFunction) -> Self {
        let socket = Socket::new(ip, remote_port, mtu_size, socket_options).expect("Error creating socket");
        let packet_buffer = PacketBuffer::new(mtu_size, socket.get_gso_size().unwrap()).expect("Error creating packet buffer");

        Client {
            mtu_discovery,
            packet_buffer,
            socket,
            history: History::new(mtu_size as u64),
            run_time_length,
            next_packet_id: 0,
            exchange_function
        }
    }

    fn send_last_message(&mut self) -> Result<(), &'static str> {
        let mut last_message_buffer: [u8; crate::LAST_MESSAGE_SIZE as usize] = [0; crate::LAST_MESSAGE_SIZE as usize];
        self.socket.send(&mut last_message_buffer, crate::LAST_MESSAGE_SIZE as usize)
    }

    fn send_messages(&mut self, packets_amount: u64) -> Result<(), &'static str> {
        match self.exchange_function {
            ExchangeFunction::Normal => self.send(),
            ExchangeFunction::Msg => self.sendmsg(),
            ExchangeFunction::Mmsg => self.sendmmsg(packets_amount),
        }
    }

    fn send(&mut self) -> Result<(), &'static str> {
        self.next_packet_id += self.packet_buffer.add_packet_ids(self.next_packet_id)?;
        let buffer_length = self.packet_buffer.get_buffer_length();

        match self.socket.send(&mut self.packet_buffer.get_buffer_pointer() , buffer_length) {
            Ok(_) => {
                self.history.amount_datagrams += 1;
                trace!("Sent datagram to remote host");
                Ok(())
            },
            Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
            Err(x) => Err(x) 
        }
    }

    fn sendmsg(&mut self) -> Result<(), &'static str> {
        self.next_packet_id += self.packet_buffer.add_packet_ids(self.next_packet_id)?;

        let buffer_length = self.packet_buffer.get_buffer_length();
        let msghdr = util::create_msghdr(&mut self.packet_buffer.get_buffer_pointer(), buffer_length);
        debug!("Sending message with buffer size {} and packet number {}",  self.packet_buffer.get_buffer_length(), self.history.amount_datagrams);
        debug!("Trying to send message with msghdr length: {}, iov_len: {}", msghdr.msg_iovlen, unsafe {*msghdr.msg_iov}.iov_len);
        trace!("Trying to send message with iov_buffer: {:?}", unsafe { std::slice::from_raw_parts((*msghdr.msg_iov).iov_base as *const u8, (*msghdr.msg_iov).iov_len)});

        match self.socket.sendmsg(&msghdr) {
            Ok(_) => {
                self.history.amount_datagrams += 1;
                trace!("Sent datagram to remote host");
                Ok(())
            },
            Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
            Err(x) => Err(x) 
        }
    }

    fn sendmmsg(&mut self, packets_amount: u64) -> Result<(), &'static str> {
        error!("Not yet implemented: {}!!!!", packets_amount);
        Ok(())
    }
}

impl Node for Client {
    fn run(&mut self) -> Result<(), &'static str> {
        info!("Current mode: client");
        debug!("Client: {:?}", self);
        self.packet_buffer.fill_with_repeating_pattern();
        self.socket.connect().expect("Error connecting to remote host"); 
    
        if self.mtu_discovery {
            info!("Set buffer size to MTU");
            let dynamic_mtu_size = self.socket.get_mtu().expect("Error getting dynamically the socket MTU") as usize;
            self.packet_buffer = PacketBuffer::new(dynamic_mtu_size, self.socket.get_gso_size().unwrap()).expect("Error creating packet buffer");
            self.history.datagram_size = dynamic_mtu_size as u64;
        }
    
        self.history.start_time = Instant::now();

        info!("Start measurement...");
    
        while self.history.start_time.elapsed().as_secs() < self.run_time_length {
            match self.send_messages(1) {
                Ok(_) => {},
                Err(x) => {
                    error!("Error sending message! Aborting measurement...");
                    return Err(x);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
        
        match self.send_last_message() {
            Ok(_) => { 
                self.history.end_time = Instant::now();
                debug!("...finished measurement");
                self.history.print();
                Ok(())
            },
            Err(_) => Err("Error sending last message"),
        }
    }
}