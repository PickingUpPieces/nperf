
use std::net::Ipv4Addr;
use std::time::Instant;
use log::{debug, error, info, trace};

use crate::net::socket_options::SocketOptions;
use crate::util::{self, ExchangeFunction, IOModel};
use crate::net::socket::Socket;
use crate::util::history::History;
use crate::util::packet_buffer::PacketBuffer;
use super::Node;

pub struct Server {
    packet_buffer: Vec<PacketBuffer>,
    socket: Socket,
    _run_infinite: bool,
    first_packet_received: bool,
    next_packet_id: u64,
    history: History,
    exchange_function: ExchangeFunction
}

impl Server {
    pub fn new(ip: Ipv4Addr, local_port: u16, mss: u32, datagram_size: u32, packet_buffer_size: usize, socket_options: SocketOptions, run_infinite: bool, exchange_function: ExchangeFunction) -> Server {
        let socket = Socket::new(ip, local_port, socket_options).expect("Error creating socket");
        let packet_buffer = Vec::from_iter((0..packet_buffer_size).map(|_| PacketBuffer::new(mss, datagram_size).expect("Error creating packet buffer")));

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
        match self.socket.recv(self.packet_buffer[0].get_buffer_pointer()) {
            Ok(amount_received_bytes) => {
                if !self.first_packet_received {
                    info!("First packet received!");
                    self.first_packet_received = true;
                    self.history.start_time = Instant::now();
                }

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                self.next_packet_id += util::process_packet(self.packet_buffer[0].get_buffer_pointer(), self.next_packet_id, &mut self.history);
                self.history.amount_datagrams += 1;
                self.history.amount_data_bytes += amount_received_bytes;
                Ok(())
            },
            Err("EAGAIN") => Ok(()),
            Err(x) => Err(x)
        }
    }

    fn recvmsg(&mut self) -> Result<(), &'static str> {
        let mut msghdr = self.packet_buffer[0].create_msghdr();
        self.packet_buffer[0].add_cmsg_buffer(&mut msghdr);

        match self.socket.recvmsg(&mut msghdr) {
            Ok(amount_received_bytes) => {
                if !self.first_packet_received {
                    info!("First packet received!");
                    self.first_packet_received = true;
                    self.history.start_time = Instant::now();
                }

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                let absolut_packets_received;
                (self.next_packet_id, absolut_packets_received) = util::process_packet_msghdr(&mut msghdr, amount_received_bytes, self.next_packet_id, &mut self.history);
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
        // TODO: Create vector once and reuse it
        let mut mmsghdr_vec = util::create_mmsghdr_vec(&mut self.packet_buffer, true);

        match self.socket.recvmmsg(&mut mmsghdr_vec) {
            Ok(amount_received_mmsghdr) => { 
                if amount_received_mmsghdr == 0 {
                    debug!("No packets received during this recvmmsg call");
                    return Ok(());
                }

                if !self.first_packet_received {
                    info!("First packet received!");
                    self.first_packet_received = true;
                    self.history.start_time = Instant::now();
                }

                // This is not very precise, since if more than one packet is received, the amount of bytes is not correct
                let amount_received_bytes = util::get_total_bytes(&mmsghdr_vec, amount_received_mmsghdr);
                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    return Err("LAST_MESSAGE_RECEIVED");
                }

                let mut absolut_datagrams_received = 0;
                for (index, mmsghdr) in mmsghdr_vec.iter_mut().enumerate() {
                    if index >= amount_received_mmsghdr {
                        break;
                    }
                    let msghdr = &mut mmsghdr.msg_hdr;
                    let msghdr_bytes = mmsghdr.msg_len as usize;

                    let datagrams_received;
                    (self.next_packet_id, datagrams_received) = util::process_packet_msghdr(msghdr, msghdr_bytes, self.next_packet_id, &mut self.history);
                    absolut_datagrams_received += datagrams_received;
                }
                // TODO: Check if all packets were sent successfully
                self.history.amount_datagrams += absolut_datagrams_received;
                self.history.amount_data_bytes += amount_received_bytes;
                trace!("Sent {} msg_hdr to remote host", amount_received_mmsghdr);
                Ok(())
            },
            Err("EAGAIN") => {
                Ok(())
            },
            Err(x) => Err(x)
        }
    }
}

impl Node for Server { 
    fn run(&mut self, io_model: IOModel) -> Result<(), &'static str>{
        info!("Current mode: server");
        self.socket.bind().expect("Error binding socket");

        info!("Start server loop...");
        self.socket.wait_for_data().expect("Error waiting for data");

        let result_loop = match io_model {
            IOModel::BusyWaiting => self.loop_busy_waiting(),
            IOModel::Select => self.loop_select(),
            IOModel::Poll => self.loop_poll(),
        };
        self.socket.close()?;

        result_loop?;

        self.history.end_time = Instant::now() - std::time::Duration::from_millis(200); // REMOVE THIS, if you remove the sleep in the client, before sending last message, as well
        debug!("Finished receiving data from remote host");
        self.history.print();
        Ok(())
    }

    fn loop_busy_waiting(&mut self) -> Result<(), &'static str> {
        loop {
            match self.recv_messages() {
                Ok(_) => {},
                Err("LAST_MESSAGE_RECEIVED") => {
                    return Ok(())
                },
                Err(x) => {
                    error!("Error receiving message! Aborting measurement...");
                    return Err(x)
                }
            }
        }
    }

    fn loop_select(&mut self) -> Result<(), &'static str> {
        todo!()
    }

    fn loop_poll(&mut self) -> Result<(), &'static str> {
        todo!()
    }
}
