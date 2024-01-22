use std::net::Ipv4Addr;
use std::time::Instant;
use log::trace;
use log::{debug, error, info};

use crate::net::socket_options::SocketOptions;
use crate::util::{self, ExchangeFunction};
use crate::net::socket::Socket;
use crate::util::History;

use super::Node;

pub struct Client {
    mtu_discovery: bool,
    buffer: Vec<u8>,
    buffer_len: usize,
    socket: Socket,
    history: History,
    run_time_length: u64,
    exchange_function: ExchangeFunction,
}

impl Node for Client {
   
    fn run(&mut self) -> Result<(), &'static str> {
        info!("Current mode: client");
        util::fill_buffer_with_repeating_pattern(&mut self.buffer);
        self.socket.connect().expect("Error connecting to remote host"); 
    
        if self.mtu_discovery {
            info!("Set buffer size to MTU");
            self.buffer = util::create_buffer_dynamic(&mut self.socket);
            self.history.datagram_size = self.buffer.len() as u64;
        }
    
        self.history.start_time = Instant::now();
        self.buffer_len = self.buffer.len();

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


impl Client {
    pub fn new(ip: Ipv4Addr, remote_port: u16, mtu_size: usize, mtu_discovery: bool, socket_options: SocketOptions, run_time_length: u64, exchange_function: ExchangeFunction) -> Self {
        let socket = Socket::new(ip, remote_port, mtu_size, socket_options).expect("Error creating socket");

        Client {
            mtu_discovery,
            buffer: vec![0; mtu_size],
            buffer_len: mtu_size,
            socket,
            history: History::new(mtu_size as u64),
            run_time_length,
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
        util::prepare_packet(self.history.amount_datagrams, &mut self.buffer);

        match self.socket.send(&mut self.buffer, self.buffer_len) {
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
        util::prepare_packet(self.history.amount_datagrams, &mut self.buffer);

        let mut msghdr = util::create_msghdr(&mut self.buffer, self.buffer_len);
        debug!("Sending message with buffer size {} and packet number {}", self.buffer_len, self.history.amount_datagrams);
        debug!("Trying to receive message with msghdr length: {}, iov_len: {}", msghdr.msg_iovlen, unsafe {*msghdr.msg_iov}.iov_len);
        trace!("Trying to receive message with iov_buffer: {:?}", unsafe { std::slice::from_raw_parts((*msghdr.msg_iov).iov_base as *const u8, (*msghdr.msg_iov).iov_len)});

        match self.socket.sendmsg(msghdr) {
            Ok(_) => {
                self.history.amount_datagrams += 1;
                trace!("Sent datagram to remote host");
                util::destroy_msghdr(&mut msghdr);
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
