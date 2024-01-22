
use std::net::Ipv4Addr;
use std::time::Instant;
use log::{debug, error, info};

use crate::net::socket_options::SocketOptions;
use crate::util::{self, ExchangeFunction};
use crate::net::socket::Socket;
use crate::util::History;
use super::Node;

pub struct Server {
    mtu_discovery: bool,
    buffer: Vec<u8>,
    socket: Socket,
    _run_infinite: bool,
    first_packet_received: bool,
    next_packet_id: u64,
    history: History,
    exchange_function: ExchangeFunction
}

impl Node for Server { 
    fn run(&mut self) -> Result<(), &'static str>{
        info!("Current mode: server");
        self.socket.bind().expect("Error binding socket");

        info!("Start server loop...");
        loop {
            match self.recv_messages() {
                Ok(_) => {},
                Err("LAST_MESSAGE_RECEIVED") => {
                    break;
                },
                Err(x) => {
                    error!("Error receiving message! Aborting measurement...");
                    return Err(x);
                }
            }
        }
        self.history.end_time = Instant::now();
        debug!("Finished receiving data from remote host");
        self.history.print();
        Ok(())
    }
}


impl Server {
    pub fn new(ip: Ipv4Addr, local_port: u16, mtu_size: usize, mtu_discovery: bool, socket_options: SocketOptions, run_infinite: bool, exchange_function: ExchangeFunction) -> Server {
        let socket = Socket::new(ip, local_port, mtu_size, socket_options).expect("Error creating socket");

        Server {
            mtu_discovery,
            buffer: vec![0; mtu_size],
            socket,
            _run_infinite: run_infinite,
            first_packet_received: false,
            next_packet_id: 0,
            history: History::new(mtu_size as u64),
            exchange_function
        }
    }

    fn recv_messages(&mut self) -> Result<(), &'static str> {
        match self.exchange_function {
            ExchangeFunction::Normal => self.recv(),
            ExchangeFunction::Msg => self.recvmsg(),
            ExchangeFunction::Mmsg => self.recvmmsg(),
        }
    }

    fn recv(&mut self) -> Result<(), &'static str> {
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

                self.history.amount_datagrams += 1;

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                self.next_packet_id += util::process_packet(&mut self.buffer, self.next_packet_id, &mut self.history);
                Ok(())
            },
            Err("EAGAIN") => Ok(()),
            Err(x) => Err(x)
        }
    }

    fn recvmsg(&mut self) -> Result<(), &'static str> {
        util::prepare_packet(self.history.amount_datagrams, &mut self.buffer);
        let buffer_len = self.buffer.len();
        let mut msghdr = util::create_msghdr(&mut self.buffer, buffer_len);
        debug!("Sending message with msghdr length: {}", msghdr.msg_iovlen);

        match self.socket.recvmsg(&mut msghdr) {
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
                debug!("Received {} bytes in {} packages", amount_received_bytes, msghdr.msg_iovlen); 

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                let absolut_packets_received;
                (self.next_packet_id, absolut_packets_received) = util::process_packet_msghdr(&mut msghdr, self.next_packet_id, &mut self.history);
                self.history.amount_datagrams += absolut_packets_received;
                debug!("Received {} packets, and next packet id should be {}", absolut_packets_received, self.next_packet_id);
                Ok(())
            },
            Err("EAGAIN") => Ok(()),
            Err(x) => Err(x)
        }
    }

    fn recvmmsg(&mut self) -> Result<(), &'static str> {
        error!("Not implemented yet!");
        Ok(())
    }


}