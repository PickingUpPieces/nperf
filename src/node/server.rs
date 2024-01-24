
use std::net::Ipv4Addr;
use std::time::Instant;
use log::{debug, error, info, trace};

use crate::net::socket_options::SocketOptions;
use crate::util::{self, ExchangeFunction};
use crate::net::socket::Socket;
use crate::util::history::History;
use crate::util::packet_buffer::PacketBuffer;
use super::Node;

#[derive(Debug)]
pub struct Server {
    mtu_discovery: bool,
    packet_buffer: PacketBuffer,
    socket: Socket,
    _run_infinite: bool,
    first_packet_received: bool,
    next_packet_id: u64,
    history: History,
    exchange_function: ExchangeFunction
}

impl Server {
    pub fn new(ip: Ipv4Addr, local_port: u16, mtu_size: usize, mtu_discovery: bool, socket_options: SocketOptions, run_infinite: bool, exchange_function: ExchangeFunction) -> Server {
        let socket = Socket::new(ip, local_port, mtu_size, socket_options).expect("Error creating socket");
        let packet_buffer = PacketBuffer::new(mtu_size, socket.get_gso_size().unwrap()).expect("Error creating packet buffer");

        Server {
            mtu_discovery,
            packet_buffer,
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
        match self.socket.recv(&mut self.packet_buffer.get_buffer_pointer()) {
            Ok(amount_received_bytes) => {
                if self.first_packet_received == false {
                    self.first_packet_received = true;
                    info!("First packet received!");

                    if self.mtu_discovery {
                        // FIXME: getting the IP_MTU from getsockopt throws an error, therefore don't use it for now
                        info!("Set buffer size to MTU");
                        let dynamic_mtu_size = self.socket.get_mtu().expect("Error getting dynamically the socket MTU") as usize;
                        self.packet_buffer = PacketBuffer::new(dynamic_mtu_size, self.socket.get_gso_size()?).expect("Error creating packet buffer");
                        self.history.datagram_size = dynamic_mtu_size as u64;
                    }

                    self.history.start_time = Instant::now();
                }

                self.history.amount_datagrams += 1;

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                self.next_packet_id += util::process_packet(&mut self.packet_buffer.get_buffer_pointer(), self.next_packet_id, &mut self.history);
                self.history.amount_data_bytes += amount_received_bytes;
                Ok(())
            },
            Err("EAGAIN") => Ok(()),
            Err(x) => Err(x)
        }
    }

    fn recvmsg(&mut self) -> Result<(), &'static str> {
        let buffer_length = self.packet_buffer.get_buffer_length();
        let mut msghdr = util::create_msghdr(&mut self.packet_buffer.get_buffer_pointer(), buffer_length);
        util::add_cmsg_buffer(&mut msghdr);
        debug!("Trying to receive message with msghdr length: {}, iov_len: {}", msghdr.msg_iovlen, unsafe {*msghdr.msg_iov}.iov_len);
        trace!("Trying to receive message with iov_buffer: {:?}", unsafe { std::slice::from_raw_parts((*msghdr.msg_iov).iov_base as *const u8, (*msghdr.msg_iov).iov_len)});

        match self.socket.recvmsg(&mut msghdr) {
            Ok(amount_received_bytes) => {
                if self.first_packet_received == false {
                    self.first_packet_received = true;
                    info!("First packet received!");

                    if self.mtu_discovery {
                        // FIXME: getting the IP_MTU from getsockopt throws an error, therefore don't use it for now
                        info!("Set buffer size to MTU");
                        let dynamic_mtu_size = self.socket.get_mtu().expect("Error getting dynamically the socket MTU") as usize;
                        self.packet_buffer = PacketBuffer::new(dynamic_mtu_size, self.socket.get_gso_size().unwrap()).expect("Error creating packet buffer");
                        self.history.datagram_size = dynamic_mtu_size as u64;
                    }
                    self.history.start_time = Instant::now();
                }
                debug!("Received {} bytes in {} packages", amount_received_bytes, msghdr.msg_iovlen); 
                trace!("Received message with iov_buffer: {:?}", unsafe { std::slice::from_raw_parts((*msghdr.msg_iov).iov_base as *const u8, (*msghdr.msg_iov).iov_len)});

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                let absolut_packets_received;
                (self.next_packet_id, absolut_packets_received) = self.packet_buffer.process_packet_msghdr(&mut msghdr, amount_received_bytes, self.next_packet_id, &mut self.history);
                self.history.amount_datagrams += absolut_packets_received;
                self.history.amount_data_bytes += amount_received_bytes;
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

impl Node for Server { 
    fn run(&mut self) -> Result<(), &'static str>{
        info!("Current mode: server");
        debug!("Server: {:?}", self);
        self.socket.bind().expect("Error binding socket");

        info!("Start server loop...");
        // TODO: Implement select() to wait for incoming data

        self.socket.wait_for_data().expect("Error waiting for data");

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
        //self.socket.wait_for_data().expect("Error waiting for data");
        }
        self.history.end_time = Instant::now();
        debug!("Finished receiving data from remote host");
        self.history.print();
        Ok(())
    }
}
