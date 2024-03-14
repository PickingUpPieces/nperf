
use std::{net::Ipv4Addr, time::Instant, collections::HashMap};
use log::{debug, error, info, trace};

use crate::net::{socket::Socket, MessageHeader, MessageType};
use crate::util::{self, ExchangeFunction, IOModel, statistic::*, packet_buffer::PacketBuffer};
use super::Node;

pub struct Server {
    packet_buffer: Vec<PacketBuffer>,
    socket: Socket,
    next_packet_id: u64,
    parameter: Parameter,
    measurements: HashMap<u64, Measurement>,
    exchange_function: ExchangeFunction
}

impl Server {
    pub fn new(ip: Ipv4Addr, local_port: u16, socket: Option<Socket>, parameter: Parameter) -> Server {
        let socket = socket.unwrap_or_else(|| Socket::new(ip, Some(local_port), None, parameter.socket_options).expect("Error creating socket"));
        info!("Current mode 'server' listening on {}:{} with socketID {}", ip, local_port, socket.get_socket_id());
        let packet_buffer = Vec::from_iter((0..parameter.packet_buffer_size).map(|_| PacketBuffer::new(parameter.mss, parameter.datagram_size).expect("Error creating packet buffer")));

        Server {
            packet_buffer,
            socket,
            next_packet_id: 0,
            parameter,
            measurements: HashMap::new(),
            exchange_function: parameter.exchange_function
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
                let test_id = MessageHeader::get_test_id(self.packet_buffer[0].get_buffer_pointer());
                let mtype = MessageHeader::get_message_type(self.packet_buffer[0].get_buffer_pointer());
                debug!("Received packet with test id: {}", test_id);

                self.parse_message_type(mtype, test_id)?;

                let statistic = &mut self.measurements.get_mut(&test_id).expect("Error getting statistic: test id not found").statistic;
                let datagram_size = self.packet_buffer[0].get_datagram_size() as usize;
                let amount_received_packets = util::process_packet_buffer(self.packet_buffer[0].get_buffer_pointer(), datagram_size, self.next_packet_id, statistic);
                self.next_packet_id += amount_received_packets;
                statistic.amount_datagrams += amount_received_packets;
                statistic.amount_data_bytes += amount_received_bytes;
                Ok(())
            },
            Err(x) => Err(x)
        }
    }

    fn recvmsg(&mut self) -> Result<(), &'static str> {
        let mut msghdr = self.packet_buffer[0].create_msghdr();
        self.packet_buffer[0].add_cmsg_buffer(&mut msghdr);

        match self.socket.recvmsg(&mut msghdr) {
            Ok(amount_received_bytes) => {
                let test_id = MessageHeader::get_test_id(self.packet_buffer[0].get_buffer_pointer());
                let mtype = MessageHeader::get_message_type(self.packet_buffer[0].get_buffer_pointer());

                self.parse_message_type(mtype, test_id)?;

                let statistic = &mut self.measurements.get_mut(&test_id).expect("Error getting statistic: test id not found").statistic;
                let absolut_packets_received;
                (self.next_packet_id, absolut_packets_received) = util::process_packet_msghdr(&mut msghdr, amount_received_bytes, self.next_packet_id, statistic);
                statistic.amount_datagrams += absolut_packets_received;
                statistic.amount_data_bytes += amount_received_bytes;
                debug!("Received {} packets and total {} Bytes, and next packet id should be {}", absolut_packets_received, amount_received_bytes, self.next_packet_id);

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

                let test_id = MessageHeader::get_test_id(self.packet_buffer[0].get_buffer_pointer());
                let mtype = MessageHeader::get_message_type(self.packet_buffer[0].get_buffer_pointer());
                let amount_received_bytes = util::get_total_bytes(&mmsghdr_vec, amount_received_mmsghdr);

                self.parse_message_type(mtype, test_id)?;

                let statistic = &mut self.measurements.get_mut(&test_id).expect("Error getting statistic: test id not found").statistic;
                let mut absolut_datagrams_received = 0;

                for (index, mmsghdr) in mmsghdr_vec.iter_mut().enumerate() {
                    if index >= amount_received_mmsghdr {
                        break;
                    }
                    let msghdr = &mut mmsghdr.msg_hdr;
                    let msghdr_bytes = mmsghdr.msg_len as usize;

                    let datagrams_received;
                    (self.next_packet_id, datagrams_received) = util::process_packet_msghdr(msghdr, msghdr_bytes, self.next_packet_id, statistic);
                    absolut_datagrams_received += datagrams_received;
                }

                statistic.amount_datagrams += absolut_datagrams_received;
                statistic.amount_data_bytes += amount_received_bytes;
                trace!("Sent {} msg_hdr to remote host", amount_received_mmsghdr);
                Ok(())
            },
            Err(x) => Err(x)
        }
    }

    fn parse_message_type(&mut self, mtype: MessageType, test_id: u64) -> Result<(), &'static str> {
        match mtype {
            MessageType::INIT => {
                info!("INIT packet received from test {}!", test_id);
                self.measurements.insert(test_id, Measurement::new(self.parameter));
                Err("INIT_MESSAGE_RECEIVED")
            },
            MessageType::MEASUREMENT => { 
                let measurement = if let Some(x) = self.measurements.get_mut(&test_id) {
                    x
                } else {
                    self.measurements.insert(test_id, Measurement::new(self.parameter));
                    self.measurements.get_mut(&test_id).expect("Error getting statistic in measurement message: test id not found")
                };
                if !measurement.first_packet_received {
                    info!("First packet received from test {}!", test_id);
                    measurement.start_time = Instant::now();
                    measurement.first_packet_received = true;
                }
                Ok(())
            },
            MessageType::LAST => {
                info!("LAST packet received from test {}!", test_id);
                let measurement = self.measurements.get_mut(&test_id).expect("Error getting statistic in last message: test id not found");
                let end_time = Instant::now() - std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE); // REMOVE THIS, if you remove the sleep in the client, before sending last message, as well
                measurement.last_packet_received = true;
                measurement.statistic.set_test_duration(measurement.start_time, end_time);
                measurement.statistic.calculate_statistics();
                Err("LAST_MESSAGE_RECEIVED")
            }
        }
    }
}

impl Node for Server { 
    fn run(&mut self, io_model: IOModel) -> Result<Statistic, &'static str>{
        if !self.parameter.single_socket {
            self.socket.bind().expect("Error binding to local port");
        }

        info!("Start server loop...");
        let mut read_fds: libc::fd_set = unsafe { self.socket.create_fdset() };
        self.socket.select(Some(&mut read_fds), None).expect("Error waiting for data");

        let mut statistic = Statistic::new(self.parameter);

        'outer: loop {
            match self.recv_messages() {
                Ok(_) => {},
                Err("EAGAIN") => {
                    statistic.amount_io_model_syscalls += 1;
                    match io_model {
                        IOModel::BusyWaiting => Ok(()),
                        IOModel::Select => self.loop_select(),
                        IOModel::Poll => self.loop_poll(),
                    }?;
                },
                Err("LAST_MESSAGE_RECEIVED") => {
                    for (_, measurement) in self.measurements.iter() {
                        if !measurement.last_packet_received {
                            info!("Last message received, but not all measurements are finished!");
                            continue 'outer;
                        } 
                    };
                    info!("Last message received and all measurements are finished!");
                    break 'outer;
                },
                Err("INIT_MESSAGE_RECEIVED") => {
                    continue;
                },
                Err(x) => {
                    error!("Error receiving message! Aborting measurement...");
                    return Err(x)
                }
            }
            statistic.amount_syscalls += 1;
        }

        if !self.parameter.single_socket {
            self.socket.close()?;
        }

        debug!("Finished receiving data from remote host");
        // Fold over all statistics, and calculate the final statistic
        let statistic = self.measurements.iter().fold(statistic, |acc: Statistic, (_, measurement)| acc + measurement.statistic);
        Ok(statistic)
    }


    fn loop_select(&mut self) -> Result<(), &'static str> {
        let mut read_fds: libc::fd_set = unsafe { self.socket.create_fdset() };

        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages 
        self.socket.select(Some(&mut read_fds), None)
    }

    fn loop_poll(&mut self) -> Result<(), &'static str> {
        let mut pollfd = self.socket.create_pollfd(libc::POLLIN);

        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages 
        self.socket.poll(&mut pollfd)
    }
}
