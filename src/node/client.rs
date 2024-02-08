use std::net::Ipv4Addr;
use std::thread::sleep;
use std::time::Instant;
use log::{trace, warn};
use log::{debug, error, info};

use crate::util::{ExchangeFunction, IOModel};
use crate::net::socket::Socket;
use crate::util::statistic::{Parameter, Statistic};
use crate::util::packet_buffer::PacketBuffer;
use crate::util;

use super::Node;

pub struct Client {
    packet_buffer: Vec<PacketBuffer>,
    socket: Socket,
    statistic: Statistic,
    run_time_length: u64,
    next_packet_id: u64,
    exchange_function: ExchangeFunction,
}

impl Client {
    pub fn new(ip: Ipv4Addr, remote_port: u16, parameter: Parameter) -> Self {
        let socket = Socket::new(ip, remote_port, parameter.socket_options).expect("Error creating socket");
        let packet_buffer = Vec::from_iter((0..parameter.packet_buffer_size).map(|_| PacketBuffer::new(parameter.mss, parameter.datagram_size).expect("Error creating packet buffer")));

        Client {
            packet_buffer,
            socket,
            statistic: Statistic::new(parameter),
            run_time_length: parameter.test_runtime_length,
            next_packet_id: 0,
            exchange_function: parameter.exchange_function
        }
    }

    fn send_last_message(&mut self) -> Result<usize, &'static str> {
        let last_message_buffer: [u8; crate::LAST_MESSAGE_SIZE] = [0; crate::LAST_MESSAGE_SIZE];
        self.socket.send(&last_message_buffer, crate::LAST_MESSAGE_SIZE)
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
        let buffer_length = self.packet_buffer[0].get_buffer_length();

        match self.socket.send(self.packet_buffer[0].get_buffer_pointer() , buffer_length) {
            Ok(amount_send_bytes) => {
                // For UDP, either the whole datagram is sent or nothing (due to an error e.g. full buffer). So we can assume that the whole datagram was sent.
                self.statistic.amount_datagrams += amount_datagrams;
                self.statistic.amount_data_bytes += amount_send_bytes;
                trace!("Sent datagram to remote host");
                Ok(())
            },
            Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
            Err(x) => Err(x) 
        }
    }

    fn sendmsg(&mut self) -> Result<(), &'static str> {
        let amount_datagrams = self.add_packet_ids()?;
        let msghdr = self.packet_buffer[0].create_msghdr();

        match self.socket.sendmsg(&msghdr) {
            Ok(amount_sent_bytes) => {
                // Since we are using UDP, we can assume that the whole datagram was sent like in send().
                self.statistic.amount_datagrams += amount_datagrams;
                self.statistic.amount_data_bytes += amount_sent_bytes;
                trace!("Sent datagram to remote host");
                Ok(())
            },
            Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
            Err(x) => Err(x) 
        }
    }

    fn sendmmsg(&mut self) -> Result<(), &'static str> {
        let amount_datagrams = self.add_packet_ids()?;
        let mut mmsghdr_vec = util::create_mmsghdr_vec(&mut self.packet_buffer, false);

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
                self.statistic.amount_data_bytes += util::get_total_bytes(&mmsghdr_vec, amount_sent_mmsghdr, self.packet_buffer[0].get_buffer_length());
                trace!("Sent {} msg_hdr to remote host", amount_sent_mmsghdr);
                Ok(())
            },
            Err("ECONNREFUSED") => Err("Start the server first! Abort measurement..."),
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


    fn fill_packet_buffers_with_repeating_pattern(&mut self) {
        for packet_buffer in self.packet_buffer.iter_mut() {
            packet_buffer.fill_with_repeating_pattern();
        }
    }
}


impl Node for Client {
    fn run(&mut self, io_model: IOModel) -> Result<(), &'static str> {
        info!("Current mode: client");
        self.fill_packet_buffers_with_repeating_pattern(); 
        self.socket.connect().expect("Error connecting to remote host"); 

        if let Ok(mss) = self.socket.get_mss() {
            info!("On the current socket the MSS is {}", mss);
        }
        
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
        sleep(std::time::Duration::from_millis(200));

        let end_time = match self.send_last_message() {
            Ok(_) => { 
                debug!("...finished measurement");
                Ok(Instant::now() - std::time::Duration::from_millis(200)) // REMOVE THIS, if you remove the sleep above as well
            },
            Err(_) => Err("Error sending last message"),
        };

        self.statistic.set_test_duration(start_time, end_time?);
        self.statistic.print();
        Ok(())
    }


    fn loop_select(&mut self) -> Result<(), &'static str> {
        let mut write_fds: libc::fd_set = unsafe { self.socket.create_fdset() };

        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages 
        self.socket.select(None, Some(&mut write_fds))
    }

    fn loop_poll(&mut self) -> Result<(), &'static str> {
        let mut pollfd = self.socket.create_pollfd(libc::POLLOUT);

        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages 
        self.socket.poll(&mut pollfd)
    }
}