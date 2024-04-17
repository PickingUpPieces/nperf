use std::net::SocketAddrV4;
use std::{thread::sleep, time::Instant};
use log::{debug, trace, info, warn, error};

use crate::net::{MessageHeader, MessageType, socket::Socket};
use crate::util::{self, ExchangeFunction, IOModel, statistic::*, packet_buffer::PacketBuffer};
use crate::DEFAULT_CLIENT_IP;
use super::Node;

pub struct Client {
    test_id: u64,
    packet_buffer: Vec<PacketBuffer>,
    socket: Socket,
    statistic: Statistic,
    run_time_length: u64,
    next_packet_id: u64,
    exchange_function: ExchangeFunction,
}

impl Client {
    pub fn new(test_id: u64, local_port: Option<u16>, sock_address_out: SocketAddrV4, socket: Option<Socket>, parameter: Parameter) -> Self {
        let socket = if socket.is_none() {
            let mut socket: Socket = Socket::new(parameter.socket_options).expect("Error creating socket");
            if let Some(port) = local_port {
                socket.bind(SocketAddrV4::new(DEFAULT_CLIENT_IP, port)).expect("Error binding socket");
            }
            socket.connect(sock_address_out).expect("Error connecting to remote host");
            socket
        } else {
            let mut socket = socket.unwrap();
            socket.set_sock_addr_out(sock_address_out); // Set socket address out for the remote host
            socket
        };

        info!("Current mode 'client' sending to remote host {}:{} from {}:{} with test ID {} on socketID {}", sock_address_out.ip(), sock_address_out.port(), DEFAULT_CLIENT_IP, local_port.unwrap_or(0), test_id, socket.get_socket_id());
        let mut packet_buffer = Vec::from_iter((0..parameter.packet_buffer_size).map(|_| PacketBuffer::new(parameter.mss, parameter.datagram_size).expect("Error creating packet buffer")));
        Self::fill_packet_buffers_with_repeating_pattern(&mut packet_buffer); 
        Self::add_message_headers(&mut packet_buffer, test_id);

        Client {
            test_id,
            packet_buffer,
            socket,
            statistic: Statistic::new(parameter),
            run_time_length: parameter.test_runtime_length,
            next_packet_id: 0,
            exchange_function: parameter.exchange_function
        }
    }

    fn send_control_message(&mut self, mtype: MessageType) -> Result<(), &'static str> {
        let header = MessageHeader::new(mtype, self.test_id, 0);
        debug!("Coordination message: {:?}", header);

        let packet_buffer = PacketBuffer::new(header.len() as u32, header.len() as u32);

        if let Some(mut packet_buffer) = packet_buffer {
            packet_buffer.copy_buffer(header.serialize());
            let sockaddr = self.socket.get_sockaddr_out().unwrap();
            packet_buffer.set_address(sockaddr);
            let msghdr = packet_buffer.get_msghdr();

            match self.socket.sendmsg(msghdr) {
                Ok(_) => { Ok(()) },
                Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
                Err(x) => Err(x)
            }
        } else {
            Err("Error creating buffer")
        }
    }

    fn send_messages(&mut self) -> Result<(), &'static str> {
        match self.exchange_function {
            ExchangeFunction::Normal => self.send(),
            ExchangeFunction::Msg => self.sendmsg(),
            ExchangeFunction::Mmsg => self.sendmmsg(),
        }
    }

    fn send(&mut self) -> Result<(), &'static str> {
        let amount_datagrams = self.add_packet_ids()?;

        // Only one buffer is used, so we can directly access the first element
        let buffer_length = self.packet_buffer[0].get_buffer_length();
        let buffer_pointer = self.packet_buffer[0].get_buffer_pointer();

        match self.socket.send(buffer_pointer , buffer_length) {
            Ok(amount_send_bytes) => {
                // For UDP, either the whole datagram is sent or nothing (due to an error e.g. full buffer). So we can assume that the whole datagram was sent.
                self.statistic.amount_datagrams += amount_datagrams;
                self.statistic.amount_data_bytes += amount_send_bytes;
                trace!("Sent datagram to remote host");
                Ok(())
            },
            Err("EAGAIN") => {
                // Reset next_packet_id to the last packet_id that was sent
                self.next_packet_id -= amount_datagrams;
                Ok(())
            },
            Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
            Err(x) => Err(x) 
        }
    }

    fn sendmsg(&mut self) -> Result<(), &'static str> {
        let amount_datagrams = self.add_packet_ids()?;

        // Only one buffer is used, so we can directly access the first element
        let msghdr = self.packet_buffer[0].get_msghdr();

        match self.socket.sendmsg(msghdr) {
            Ok(amount_sent_bytes) => {
                // Since we are using UDP, we can assume that the whole datagram was sent like in send().
                self.statistic.amount_datagrams += amount_datagrams;
                self.statistic.amount_data_bytes += amount_sent_bytes;
                trace!("Sent datagram to remote host");
                Ok(())
            },
            Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
            Err("EAGAIN") => {
                // Reset next_packet_id to the last packet_id that was sent
                self.next_packet_id -= amount_datagrams;
                Ok(())
            },
            Err(x) => Err(x) 
        }
    }

    fn sendmmsg(&mut self) -> Result<(), &'static str> {
        let amount_datagrams = self.add_packet_ids()?;
        let mut mmsghdr_vec = util::create_mmsghdr_vec(&mut self.packet_buffer);

        match self.socket.sendmmsg(&mut mmsghdr_vec) {
            Ok(amount_sent_mmsghdr) => { 
                if amount_sent_mmsghdr != self.packet_buffer.len() {
                    // Check until which index the packets were sent. Either all packets in a msghdr are sent or none.
                    // Reset self.next_packet_id to the last packet_id that was sent
                    warn!("Not all packets were sent! Sent: {}, Expected: {}", amount_sent_mmsghdr, amount_datagrams);
                    let packet_amount_per_msghdr = self.packet_buffer[0].get_packet_amount();
                    let amount_not_sent_packets = (self.packet_buffer.len() - amount_sent_mmsghdr) * packet_amount_per_msghdr;
                    self.next_packet_id -= amount_not_sent_packets as u64;
                }
                self.statistic.amount_datagrams += (amount_sent_mmsghdr * self.packet_buffer[0].get_packet_amount()) as u64;
                self.statistic.amount_data_bytes += util::get_total_bytes(&mmsghdr_vec, amount_sent_mmsghdr);
                trace!("Sent {} msg_hdr to remote host", amount_sent_mmsghdr);
                Ok(())
            },
            Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
            Err("EAGAIN") => {
                // Reset next_packet_id to the last packet_id that was sent
                self.next_packet_id -= amount_datagrams;
                Ok(())
            },
            Err(x) => Err(x)
        }
    }


    fn add_packet_ids(&mut self) -> Result<u64, &'static str> {
        let mut total_amount_used_packet_ids: u64 = 0;

        for packet_buffer in self.packet_buffer.iter_mut() {
            let amount_used_packet_ids = packet_buffer.add_packet_ids(self.next_packet_id)?;
            self.next_packet_id += amount_used_packet_ids;
            total_amount_used_packet_ids += amount_used_packet_ids;
        }

        debug!("Added packet IDs to buffer! Used packet IDs: {}, Next packet ID: {}", total_amount_used_packet_ids, self.next_packet_id);
        // Return amount of used packet IDs
        Ok(total_amount_used_packet_ids)
    }

    fn add_message_headers(packet_buffer: &mut Vec<PacketBuffer>, test_id: u64) {
        for packet_buffer in packet_buffer.iter_mut() {
            packet_buffer.add_message_header(test_id, 0).expect("Error adding message header");
        }
    }

    fn fill_packet_buffers_with_repeating_pattern(packet_buffer: &mut Vec<PacketBuffer>) {
        for packet_buffer in packet_buffer.iter_mut() {
            packet_buffer.fill_with_repeating_pattern();
        }
    }
}


impl Node for Client {
    fn run(&mut self, io_model: IOModel) -> Result<Statistic, &'static str> {
        // Set outgoing address for individual multiplexing
        if self.statistic.parameter.multiplex_port == MultiplexPort::Sharing && self.statistic.parameter.multiplex_port_server == MultiplexPort::Individual {
            if let Some(sockaddr) = self.socket.get_sockaddr_out() {
                self.packet_buffer.iter_mut().for_each(|packet_buffer| packet_buffer.set_address(sockaddr));
            } else {
                return Err("No socket address out set on socket! Aborting measurement...");
            }
        }

        if let Ok(mss) = self.socket.get_mss() {
            info!("On the current socket the MSS is {}", mss);
        }
        
        self.send_control_message(MessageType::INIT)?;

        // Ensures that the server is ready to receive messages
        sleep(std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE));

        let start_time = Instant::now();
        info!("Start measurement...");

        while start_time.elapsed().as_secs() < self.run_time_length {
            match self.send_messages() {
                Ok(_) => {},
                Err("EAGAIN") => {
                    self.statistic.amount_io_model_syscalls += 1;
                    match io_model {
                        IOModel::BusyWaiting => Ok(()),
                        IOModel::Select => self.loop_select(),
                        IOModel::Poll => self.loop_poll(),
                    }?;
                },
                Err(x) => {
                    error!("Error sending message! Aborting measurement...");
                    return Err(x)
                }
            }
            self.statistic.amount_syscalls += 1;
        }
        // Ensures that the buffers are empty again, so that the last message actually arrives at the server
        sleep(std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE));

        self.send_control_message(MessageType::LAST)?;

        let end_time = Instant::now() - std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE);

        self.statistic.set_test_duration(start_time, end_time);
        self.statistic.calculate_statistics();
        Ok(self.statistic)
    }


    fn loop_select(&mut self) -> Result<(), &'static str> {
        let mut write_fds: libc::fd_set = unsafe { self.socket.create_fdset() };

        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages 
        self.socket.select(None, Some(&mut write_fds), -1)
    }

    fn loop_poll(&mut self) -> Result<(), &'static str> {
        let mut pollfd = self.socket.create_pollfd(libc::POLLOUT);

        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages 
        self.socket.poll(&mut pollfd, -1)
    }
}