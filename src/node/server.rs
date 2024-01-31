
use std::net::Ipv4Addr;
use std::time::Instant;
use log::{debug, error, info};

use crate::net::socket_options::SocketOptions;
use crate::util::{self, ExchangeFunction};
use crate::net::socket::Socket;
use crate::util::history::History;
use crate::util::packet_buffer::PacketBuffer;
use super::Node;

pub struct Server {
    packet_buffer: PacketBuffer,
    socket: Socket,
    _run_infinite: bool,
    first_packet_received: bool,
    next_packet_id: u64,
    history: History,
    exchange_function: ExchangeFunction
}

impl Server {
    pub fn new(ip: Ipv4Addr, local_port: u16, mss: u32, datagram_size: u32, socket_options: SocketOptions, run_infinite: bool, exchange_function: ExchangeFunction) -> Server {
        let socket = Socket::new(ip, local_port, socket_options).expect("Error creating socket");
        let packet_buffer = PacketBuffer::new(mss, datagram_size).expect("Error creating packet buffer");

        Server {
            packet_buffer,
            socket,
            _run_infinite: run_infinite,
            first_packet_received: false,
            next_packet_id: 0,
            history: History::new(),
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
        match self.socket.recv(self.packet_buffer.get_buffer_pointer()) {
            Ok(amount_received_bytes) => {
                if !self.first_packet_received {
                    self.first_packet_received = true;
                    info!("First packet received!");
                    self.history.start_time = Instant::now();
                }

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                self.next_packet_id += util::process_packet(self.packet_buffer.get_buffer_pointer(), self.next_packet_id, &mut self.history);
                self.history.amount_datagrams += 1;
                self.history.amount_data_bytes += amount_received_bytes;
                Ok(())
            },
            Err("EAGAIN") => Ok(()),
            Err(x) => Err(x)
        }
    }

    fn recvmsg(&mut self) -> Result<(), &'static str> {
        let mut msghdr = self.packet_buffer.create_msghdr();
        self.packet_buffer.add_cmsg_buffer(&mut msghdr);

        match self.socket.recvmsg(&mut msghdr) {
            Ok(amount_received_bytes) => {
                if !self.first_packet_received {
                    self.first_packet_received = true;
                    info!("First packet received!");
                    self.history.start_time = Instant::now();
                }

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                let absolut_packets_received;
                (self.next_packet_id, absolut_packets_received) = self.packet_buffer.process_packet_msghdr(&mut msghdr, amount_received_bytes, self.next_packet_id, &mut self.history);
                self.history.amount_datagrams += absolut_packets_received;
                self.history.amount_data_bytes += amount_received_bytes;
                debug!("Received {} packets and total {} Bytes, and next packet id should be {}", absolut_packets_received, amount_received_bytes, self.next_packet_id);

                Ok(())
            },
            Err("EAGAIN") => {
                Ok(())
            },
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
        self.socket.bind().expect("Error binding socket");

        info!("Start server loop...");
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
