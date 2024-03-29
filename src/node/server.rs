
use std::net::SocketAddrV4;
use std::thread::{self, sleep};
use std::time::Instant;
use log::{debug, error, info, trace};

use crate::net::{socket::Socket, MessageHeader, MessageType};
use crate::util::{self, ExchangeFunction, IOModel, statistic::*, packet_buffer::PacketBuffer};
use super::Node;

const INITIAL_POLL_TIMEOUT: i32 = 10000; // in milliseconds
const IN_MEASUREMENT_POLL_TIMEOUT: i32 = 1000; // in milliseconds

pub struct Server {
    packet_buffer: Vec<PacketBuffer>,
    socket: Socket,
    next_packet_id: u64,
    parameter: Parameter,
    measurements: Vec<Measurement>,
    exchange_function: ExchangeFunction
}

impl Server {
    pub fn new(sock_address_in: SocketAddrV4, socket: Option<Socket>, parameter: Parameter) -> Server {
        let socket = if let Some(socket) = socket {
            socket
        } else {
            let mut socket: Socket = Socket::new(parameter.socket_options).expect("Error creating socket");
            socket.bind(sock_address_in).expect("Error binding to local port");
            socket
        };

        info!("Current mode 'server' listening on {}:{} with socketID {}", sock_address_in.ip(), sock_address_in.port(), socket.get_socket_id());
        let mut packet_buffer = Vec::from_iter((0..parameter.packet_buffer_size).map(|_| PacketBuffer::new(parameter.mss, parameter.datagram_size).expect("Error creating packet buffer")));
        packet_buffer.iter_mut().for_each(|packet_buffer| packet_buffer.add_cmsg_buffer());

        Server {
            packet_buffer,
            socket,
            next_packet_id: 0,
            parameter,
            measurements: Vec::new(),
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
        // Only one buffer is used, so we can directly access the first element
        let buffer_pointer = self.packet_buffer[0].get_buffer_pointer();

        match self.socket.recv(buffer_pointer) {
            Ok(amount_received_bytes) => {
                let test_id = MessageHeader::get_test_id(self.packet_buffer[0].get_buffer_pointer()) as usize;
                let mtype = MessageHeader::get_message_type(self.packet_buffer[0].get_buffer_pointer());
                debug!("Received packet with test id: {}", test_id);

                self.parse_message_type(mtype, test_id)?;

                let statistic = &mut self.measurements.get_mut(test_id).expect("Error getting statistic: test id not found").statistic;
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
        // Only one buffer is used, so we can directly access the first element
        let msghdr = self.packet_buffer[0].get_msghdr();

        match self.socket.recvmsg(msghdr) {
            Ok(amount_received_bytes) => {
                let test_id = MessageHeader::get_test_id(self.packet_buffer[0].get_buffer_pointer()) as usize;
                let mtype = MessageHeader::get_message_type(self.packet_buffer[0].get_buffer_pointer());

                self.parse_message_type(mtype, test_id)?;

                let msghdr = self.packet_buffer[0].get_msghdr();
                let statistic = &mut self.measurements.get_mut(test_id).expect("Error getting statistic: test id not found").statistic;
                let absolut_packets_received;
                (self.next_packet_id, absolut_packets_received) = util::process_packet_msghdr(msghdr, amount_received_bytes, self.next_packet_id, statistic);
                statistic.amount_datagrams += absolut_packets_received;
                statistic.amount_data_bytes += amount_received_bytes;
                debug!("Received {} packets and total {} Bytes, and next packet id should be {}", absolut_packets_received, amount_received_bytes, self.next_packet_id);

                Ok(())
            },
            Err(x) => Err(x)
        }
    }

    fn recvmmsg(&mut self) -> Result<(), &'static str> {
        let mut mmsghdr_vec = util::create_mmsghdr_vec(&mut self.packet_buffer, true);

        match self.socket.recvmmsg(&mut mmsghdr_vec) {
            Ok(amount_received_mmsghdr) => { 
                if amount_received_mmsghdr == 0 {
                    debug!("No packets received during this recvmmsg call");
                    return Ok(());
                }

                let test_id = MessageHeader::get_test_id(self.packet_buffer[0].get_buffer_pointer()) as usize;
                let mtype = MessageHeader::get_message_type(self.packet_buffer[0].get_buffer_pointer());
                let amount_received_bytes = util::get_total_bytes(&mmsghdr_vec, amount_received_mmsghdr);

                self.parse_message_type(mtype, test_id)?;

                let statistic = &mut self.measurements.get_mut(test_id).expect("Error getting statistic: test id not found").statistic;
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

    fn parse_message_type(&mut self, mtype: MessageType, test_id: usize) -> Result<(), &'static str> {
        match mtype {
            MessageType::INIT => {
                info!("{:?}: INIT packet received from test {}!", thread::current().id(), test_id);
                if self.measurements.len() <= test_id {
                    self.measurements.resize(test_id + 1, Measurement::new(self.parameter));
                }
                Err("INIT_MESSAGE_RECEIVED")
            },
            MessageType::MEASUREMENT => { 
                let measurement = if let Some(x) = self.measurements.get_mut(test_id) {
                    x
                } else {
                    if self.measurements.len() <= test_id {
                        self.measurements.resize(test_id + 1, Measurement::new(self.parameter));
                    }
                    self.measurements.get_mut(test_id).expect("Error getting statistic in measurement message: test id not found")
                };
                if !measurement.first_packet_received {
                    info!("{:?}: First packet received from test {}!", thread::current().id(), test_id);
                    measurement.start_time = Instant::now();
                    measurement.first_packet_received = true;
                }
                Ok(())
            },
            MessageType::LAST => {
                info!("{:?}: LAST packet received from test {}!", thread::current().id(), test_id);
                let measurement = self.measurements.get_mut(test_id).expect("Error getting statistic in last message: test id not found");
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
        info!("Start server loop...");
        let mut statistic = Statistic::new(self.parameter);

        let mut pollfd = self.socket.create_pollfd(libc::POLLIN);
        // TODO: Add 10s timeout -> With communication channel in future, the measure thread is only started if the client starts a measurement. Then timeout can be further reduced to 1-2s.
        match self.socket.poll(&mut pollfd, INITIAL_POLL_TIMEOUT) {
            Ok(_) => {},
            Err("TIMEOUT") => {
                // If port sharding is used, not every server thread gets packets due to the load balancing of REUSEPORT.
                // To avoid that the thread waits forever, we need to return here.
                error!("{:?}: Timeout waiting for client to send first packet!", thread::current().id());
                return Ok(statistic);
            },
            Err(x) => {
                return Err(x);
            }
        };

        'outer: loop {
            match self.recv_messages() {
                Ok(_) => {},
                Err("EAGAIN") => {
                    statistic.amount_io_model_syscalls += 1;
                    match match io_model {
                        IOModel::BusyWaiting => Ok(()),
                        IOModel::Select => self.loop_select(),
                        IOModel::Poll => self.loop_poll(),
                    } {
                        Ok(_) => {},
                        Err("TIMEOUT") => {
                            // If port sharing is used, or single connection not every thread receives the LAST message. 
                            // To avoid that the thread waits forever, we need to return here.
                            error!("{:?}: Timeout waiting for a subsequent packet from the client!", thread::current().id());
                            break 'outer;
                        },
                        Err(x) => {
                            return Err(x);
                        }
                    }
                },
                Err("LAST_MESSAGE_RECEIVED") => {
                    for measurement in self.measurements.iter() {
                        if !measurement.last_packet_received && measurement.first_packet_received {
                            debug!("{:?}: Last message received, but not all measurements are finished!", thread::current().id());
                            continue 'outer;
                        } 
                    };
                    info!("{:?}: Last message received and all measurements are finished!", thread::current().id());
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


        if self.parameter.multiplex_port_server != MultiplexPort::Sharing {
            // If a thread finishes (closes the socket) before the others, the hash mapping of SO_REUSEPORT changes. 
            // Then all threads would receive packets from other connections (test_ids).
            // Therefore, we need to wait a bit, until a thread closes its socket.
            if self.parameter.multiplex_port_server == MultiplexPort::Sharding {
                sleep(std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE * 2));
            }
            self.socket.close()?;
        }

        debug!("{:?}: Finished receiving data from remote host", thread::current().id());
        // Fold over all statistics, and calculate the final statistic
        let statistic = self.measurements.iter().fold(statistic, |acc: Statistic, measurement| acc + measurement.statistic);
        Ok(statistic)
    }


    fn loop_select(&mut self) -> Result<(), &'static str> {
        let mut read_fds: libc::fd_set = unsafe { self.socket.create_fdset() };

        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages 
        self.socket.select(Some(&mut read_fds), None, IN_MEASUREMENT_POLL_TIMEOUT)
    }

    fn loop_poll(&mut self) -> Result<(), &'static str> {
        let mut pollfd = self.socket.create_pollfd(libc::POLLIN);

        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages 
        self.socket.poll(&mut pollfd, IN_MEASUREMENT_POLL_TIMEOUT)
    }
}
