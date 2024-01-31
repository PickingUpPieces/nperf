use std::net::Ipv4Addr;
use std::thread::sleep;
use std::time::Instant;
use log::trace;
use log::{debug, error, info};

use crate::net::socket_options::SocketOptions;
use crate::util::ExchangeFunction;
use crate::net::socket::Socket;
use crate::util::history::History;
use crate::util::packet_buffer::PacketBuffer;
use crate::util;

use super::Node;

pub struct Client {
    packet_buffer: Vec<PacketBuffer>,
    socket: Socket,
    history: History,
    run_time_length: u64,
    next_packet_id: u64,
    exchange_function: ExchangeFunction,
}

impl Client {
    pub fn new(ip: Ipv4Addr, remote_port: u16, mss: u32, datagram_size: u32, packet_buffer_size: usize, socket_options: SocketOptions, run_time_length: u64, exchange_function: ExchangeFunction) -> Self {
        let socket = Socket::new(ip, remote_port, socket_options).expect("Error creating socket");
        let packet_buffer = Vec::from_iter((0..packet_buffer_size).map(|_| PacketBuffer::new(mss, datagram_size).expect("Error creating packet buffer")));

        Client {
            packet_buffer,
            socket,
            history: History::new(),
            run_time_length,
            next_packet_id: 0,
            exchange_function
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
        self.add_packet_ids()?;
        let buffer_length = self.packet_buffer[0].get_buffer_length();

        match self.socket.send(self.packet_buffer[0].get_buffer_pointer() , buffer_length) {
            Ok(amount_send_bytes) => {
                self.history.amount_datagrams += 1;
                self.history.amount_data_bytes += amount_send_bytes;
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
                // FIXME: Check if all packets were sent
                self.history.amount_datagrams += amount_datagrams;
                self.history.amount_data_bytes += amount_sent_bytes;
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
                // FIXME: Check if all packets were sent
                self.history.amount_datagrams += amount_datagrams;
                self.history.amount_data_bytes += util::get_total_bytes(&mmsghdr_vec, amount_sent_mmsghdr);
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
    fn run(&mut self) -> Result<(), &'static str> {
        info!("Current mode: client");
        self.fill_packet_buffers_with_repeating_pattern(); 
        self.socket.connect().expect("Error connecting to remote host"); 

        if let Ok(mss) = self.socket.get_mss() {
            info!("On the current socket the MSS is {}", mss);
        }
        
        self.history.start_time = Instant::now();
        info!("Start measurement...");
    
        while self.history.start_time.elapsed().as_secs() < self.run_time_length {
            match self.send_messages() {
                Ok(_) => {},
                Err(x) => {
                    error!("Error sending message! Aborting measurement...");
                    return Err(x);
                }
            }
        }

        sleep(std::time::Duration::from_millis(200));

        match self.send_last_message() {
            Ok(_) => { 
                self.history.end_time = Instant::now() - std::time::Duration::from_millis(200); // REMOVE THIS, if you remove the sleep above as well
                debug!("...finished measurement");
                self.history.print();
                Ok(())
            },
            Err(_) => Err("Error sending last message"),
        }
    }
}