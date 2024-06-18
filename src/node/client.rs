use std::net::SocketAddrV4;
use std::os::fd::RawFd;
use std::sync::mpsc;
use std::{thread::sleep, time::Instant};
use log::{debug, trace, info, warn, error};

use crate::io_uring::send::IoUringSend;
use crate::io_uring::{check_multishot_status, IoUringOperatingModes, UringMode};
use crate::net::{MessageHeader, MessageType, socket::Socket};
use crate::util::msghdr_vec::MsghdrVec;
use crate::util::packet_buffer::PacketBuffer;
use crate::util::{self, ExchangeFunction, IOModel, statistic::*, msghdr::WrapperMsghdr};
use crate::DEFAULT_CLIENT_IP;
use super::Node;

pub struct Client {
    test_id: u64,
    packet_buffer: PacketBuffer,
    socket: Socket,
    parameter: Parameter,
    io_uring_sqpoll_fd: Option<RawFd>,
    statistic: Statistic,
    run_time_length: u64,
    next_packet_id: u64,
    exchange_function: ExchangeFunction,
}

impl Client {
    pub fn new(test_id: u64, local_port: Option<u16>, sock_address_out: SocketAddrV4, socket: Option<Socket>, io_uring: Option<RawFd>, parameter: Parameter) -> Self {
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

        let packet_buffer = Self::create_packet_buffer(&parameter, test_id, &socket); 

        Client {
            test_id,
            packet_buffer,
            socket,
            parameter: parameter.clone(),
            io_uring_sqpoll_fd: io_uring,
            statistic: Statistic::new(parameter.clone()),
            run_time_length: parameter.test_runtime_length,
            next_packet_id: 0,
            exchange_function: parameter.exchange_function
        }
    }

    fn send_control_message(&mut self, mtype: MessageType) -> Result<(), &'static str> {
        let header = MessageHeader::new(mtype, self.test_id, 0);
        debug!("Coordination message: {:?}", header);

        let packet_buffer = WrapperMsghdr::new(header.len() as u32, header.len() as u32);

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
        let amount_datagrams = self.packet_buffer.add_packet_ids(self.next_packet_id, None)?;
        self.next_packet_id += amount_datagrams;

        // Only one buffer is used, so we can directly access the first element
        let buffer_length = self.packet_buffer.datagram_size();
        let buffer_pointer = self.packet_buffer.get_buffer_pointer_from_index(0).unwrap();

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
        let amount_datagrams = self.packet_buffer.add_packet_ids(self.next_packet_id, None)?;
        self.next_packet_id += amount_datagrams;

        // Only one buffer is used, so we can directly access the first element
        let msghdr = self.packet_buffer.get_msghdr_from_index(0).unwrap();

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
        let amount_datagrams = self.packet_buffer.add_packet_ids(self.next_packet_id, None)?;
        self.next_packet_id += amount_datagrams;

        match self.socket.sendmmsg(&mut self.packet_buffer.mmsghdr_vec) {
            Ok(amount_sent_mmsghdr) => { 
                let amount_packets_per_msghdr = self.packet_buffer.packets_amount_per_msghdr();

                if amount_sent_mmsghdr != self.packet_buffer.mmsghdr_vec.len() {
                    // Check until which index the packets were sent. Either all packets in a msghdr are sent or none.
                    // Reset self.next_packet_id to the last packet_id that was sent
                    warn!("Not all packets were sent! Sent: {}, Expected: {}", amount_sent_mmsghdr, amount_datagrams);
                    let amount_not_sent_packets = (self.packet_buffer.mmsghdr_vec.len() - amount_sent_mmsghdr) * amount_packets_per_msghdr;
                    self.next_packet_id -= amount_not_sent_packets as u64;
                }
                self.statistic.amount_datagrams += (amount_sent_mmsghdr * amount_packets_per_msghdr) as u64;
                self.statistic.amount_data_bytes += util::get_total_bytes(&self.packet_buffer.mmsghdr_vec, amount_sent_mmsghdr);
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

    fn create_packet_buffer(parameter: &Parameter, test_id: u64, socket: &Socket) -> PacketBuffer {
        let mut packet_buffer = MsghdrVec::new(parameter.packet_buffer_size, parameter.mss, parameter.datagram_size as usize).with_random_payload().with_message_header(test_id);

        if parameter.multiplex_port == MultiplexPort::Sharing && parameter.multiplex_port_server == MultiplexPort::Individual {
            if let Some(sockaddr) = socket.get_sockaddr_out() {
                packet_buffer = packet_buffer.with_target_address(sockaddr);
            } 
        }

        PacketBuffer::new(packet_buffer)
    }

    fn io_uring_complete_send(&mut self, io_uring_instance: &mut IoUringSend) -> Result<usize, &'static str> {
        let mut completion_count = 0;
        let amount_datagrams = self.packet_buffer.packets_amount_per_msghdr() as u64;
        let cq = io_uring_instance.get_cq();

        debug!("BEGIN io_uring_complete: Current cq len: {}. Dropped messages: {}", cq.len(), cq.overflow());

        if cq.overflow() > 0 {
            warn!("Dropped messages in completion queue: {}", cq.overflow());
        }

        // Drain completion queue events
        for cqe in cq {
            let amount_bytes = cqe.result();
            let user_data = cqe.user_data();
            debug!("Received completion event with user_data: {}, and received bytes: {}", user_data, amount_bytes); 

            match amount_bytes {
                -11 => { // libc::EAGAIN == 11
                    // If no messages are available at the socket, the receive calls wait for a message to arrive, unless the socket is nonblocking (see fcntl(2)), in which case the value -11 is returned and the external variable errno is set to EAGAIN or EWOULDBLOCK.
                    // From: https://linux.die.net/man/2/recvmsg
                    debug!("EAGAIN: No messages can be send at the socket!"); // This should not happen in io_uring with FAST_POLL
                    self.statistic.amount_omitted_datagrams += amount_datagrams as i64; // Currently we don't resend the packets
                },
                -111 => { // libc::ECONNREFUSED == 111
                    return Err("Start the server first! Abort measurement...");
                },
                _ if amount_bytes < 0 => {
                    error!("Error receiving message! Negated error code: {}", amount_bytes);
                    return Err("Failed to receive data!")
                },
                _ => { // Positive amount of bytes received
                    self.statistic.amount_datagrams += amount_datagrams;
                    self.statistic.amount_data_bytes += amount_bytes as usize;
                    completion_count += 1;
                    trace!("Sent datagram to remote host");
                }
            }
        }

        debug!("END io_uring_complete: Completed {} io_uring cqe", completion_count);
        Ok(completion_count)
    }

    fn io_uring_complete_send_zc(&mut self, io_uring_instance: &mut IoUringSend) -> Result<usize, &'static str> {
        let mut completion_count = 0;
        let amount_datagrams = self.packet_buffer.packets_amount_per_msghdr() as u64;
        let cq = io_uring_instance.get_cq();
        let mut index_pool: Vec<usize> = Vec::with_capacity(cq.len());

        debug!("BEGIN io_uring_complete: Current cq len: {}. Dropped messages: {}", cq.len(), cq.overflow());

        if cq.overflow() > 0 {
            warn!("Dropped messages in completion queue: {}", cq.overflow());
        }

        // Drain completion queue events
        for cqe in cq {
            let amount_bytes = cqe.result();
            let user_data = cqe.user_data();
            debug!("Received completion event with user_data: {}, and received bytes: {}", user_data, amount_bytes); 

            match amount_bytes {
                -11 => { // libc::EAGAIN == 11
                    // If no messages are available at the socket, the receive calls wait for a message to arrive, unless the socket is nonblocking (see fcntl(2)), in which case the value -11 is returned and the external variable errno is set to EAGAIN or EWOULDBLOCK.
                    // From: https://linux.die.net/man/2/recvmsg
                    debug!("EAGAIN: No messages can be send at the socket!"); // This should not happen in io_uring with FAST_POLL
                    index_pool.push(user_data as usize);
                    self.statistic.amount_omitted_datagrams += amount_datagrams as i64; // Currently we don't resend the packets
                },
                -111 => { // libc::ECONNREFUSED == 111
                    return Err("Start the server first! Abort measurement...");
                }
                -2147483648 => { // IORING_NOTIF_USAGE_ZC_COPIED -> Error returned if data was copied in zero copy mode
                    // Check if the error code is set in second cqe event
                    // https://github.com/axboe/liburing/blob/b68cf47a120d6b117a81ed9f7617aad13314258c/src/include/liburing/io_uring.h#L343
                    if !check_multishot_status(cqe.flags()) && cqe.flags() & crate::io_uring::IORING_CQE_F_NOTIF != 0 {
                        self.statistic.uring_copied_zc += 1;
                        debug!("Received second send zero copy cqe. Returning buffer {}", user_data);
                        index_pool.push(user_data as usize);
                        completion_count += 1; // Completion count should only be increased when the buffer is returned. Otherwise too many requests could be inflight at once.
                    }
                }
                _ if amount_bytes < 0 => {
                    error!("Error receiving message! Negated error code: {}", amount_bytes);
                    return Err("Failed to receive data!")
                },
                0 => {
                    // Send zero copy will publish two cqe events for the same buffer. The first one will have the amount of bytes sent, and confirmes that the request is queued. It has the flag IORING_CQE_F_MORE set.
                    // The second one will have 0 bytes set, and confirms that the buffer can be reused. The second message doesn't have the flag IORING_CQE_F_MORE set, but the flag IORING_CQE_F_NOTIF.
                    if !check_multishot_status(cqe.flags()) && cqe.flags() & crate::io_uring::IORING_CQE_F_NOTIF != 0 {
                        debug!("Received second send zero copy cqe. Returning buffer {}", user_data);
                        index_pool.push(user_data as usize);
                        completion_count += 1; // Completion count should only be increased when the buffer is returned. Otherwise too many requests could be inflight at once.
                    }
                },
                _ => { // Positive amount of bytes received
                    self.statistic.amount_datagrams += amount_datagrams;
                    self.statistic.amount_data_bytes += amount_bytes as usize;
                    trace!("Sent datagram to remote host");
                }
            }
        }

        // Returns used buffers to the buffer ring.
        self.packet_buffer.return_buffer_index(index_pool);

        debug!("END io_uring_complete: Completed {} io_uring cqe", completion_count);
        Ok(completion_count)
    }


    fn io_uring_loop(&mut self, start_time: Instant, tx: mpsc::Sender<Option<Statistic>>) -> Result<(), &'static str> {
        let socket_fd = self.socket.get_socket_id();
        let uring_mode = self.parameter.uring_parameter.uring_mode;
        let mut statistic_interval = StatisticInterval::new(start_time, self.parameter.output_interval, self.parameter.test_runtime_length, Statistic::new(self.parameter.clone()));
        let mut amount_inflight: usize = 0;

        match uring_mode {
            UringMode::Normal | UringMode::Zerocopy => {
                let mut io_uring_instance = crate::io_uring::send::IoUringSend::new(self.parameter.clone(), self.io_uring_sqpoll_fd)?;

                while start_time.elapsed().as_secs() < self.run_time_length {
                    self.statistic.uring_inflight_utilization[amount_inflight] += 1;
                    self.statistic.amount_io_model_calls += 1;

                    // Check if the time elapsed since the last send operation is greater than or equal to self.parameters.interval seconds
                    if self.parameter.output_interval != 0.0 && statistic_interval.last_send_instant.elapsed().as_secs_f64() >= statistic_interval.output_interval {
                        if let Some(stat) = statistic_interval.calculate_interval(self.statistic.clone()) {
                            tx.send(Some(stat)).unwrap();
                        } 
                    }

                    let submitted = io_uring_instance.fill_sq_and_submit(amount_inflight, &mut self.packet_buffer, self.next_packet_id, socket_fd)?;
                    amount_inflight += submitted;
                    self.next_packet_id += (submitted * self.packet_buffer.packets_amount_per_msghdr()) as u64;

                    match if uring_mode == UringMode::Zerocopy { self.io_uring_complete_send_zc(&mut io_uring_instance) } else { self.io_uring_complete_send(&mut io_uring_instance) } {
                        Ok(completed) => {
                            amount_inflight -= completed
                        },
                        Err("EAGAIN") => {},
                        Err(x) => {
                            error!("Error completing io_uring sqe: {}", x);
                            return Err(x);
                        }
                    };
                }
            },
            _ => return Err("Invalid io_uring mode for client"),
        }
        Ok(())
    }
}


impl Node for Client {
    fn run(&mut self, io_model: IOModel, tx: mpsc::Sender<Option<Statistic>>) -> Result<Statistic, &'static str> {
        if let Ok(mss) = self.socket.get_mss() {
            info!("On the current socket the MSS is {}", mss);
        }
        
        self.send_control_message(MessageType::INIT)?;

        // Ensures that the server is ready to receive messages
        sleep(std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE));

        let start_time = Instant::now();
        let mut statistic_interval = StatisticInterval::new(start_time, self.parameter.output_interval, self.parameter.test_runtime_length, Statistic::new(self.parameter.clone()));
        info!("Start measurement...");

        if io_model == IOModel::IoUring {
            self.io_uring_loop(start_time, tx.clone())?;
        } else {

            while start_time.elapsed().as_secs() < self.run_time_length {
                // Check if the time elapsed since the last send operation is greater than or equal to self.parameters.interval seconds
                if self.parameter.output_interval != 0.0 && statistic_interval.last_send_instant.elapsed().as_secs_f64() >= statistic_interval.output_interval {
                    if let Some(stat) = statistic_interval.calculate_interval(self.statistic.clone()) {
                        tx.send(Some(stat)).unwrap();
                    } 
                }

                match self.send_messages() {
                    Ok(_) => {},
                    Err("EAGAIN") => {
                        self.statistic.amount_io_model_calls += 1;
                        self.io_wait(io_model)?;
                    },
                    Err(x) => {
                        error!("Error sending message! Aborting measurement...");
                        return Err(x)
                    }
                }
                self.statistic.amount_syscalls += 1;
            }
        }

        // Print last interval
        if self.parameter.output_interval != 0.0 && !statistic_interval.finished() {
            if let Some(stat) = statistic_interval.calculate_interval(self.statistic.clone()) {
                tx.send(Some(stat)).unwrap();
            }
        }

        // Ensures that the buffers are empty again, so that the last message actually arrives at the server
        sleep(std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE));

        self.send_control_message(MessageType::LAST)?;

        let end_time = Instant::now() - std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE);

        self.statistic.set_test_duration(start_time, end_time);
        self.statistic.interval_id = self.statistic.parameter.test_runtime_length as f64;
        self.statistic.calculate_statistics();
        Ok(self.statistic.clone())
    }

    fn io_wait(&mut self, io_model: IOModel) -> Result<(), &'static str> {
        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call send_messages after io_wait returns
        match io_model {
            IOModel::Select => {
                let mut write_fds: libc::fd_set = unsafe { self.socket.create_fdset() };
                self.socket.select(None, Some(&mut write_fds), -1)

            },
            IOModel::Poll => {
                let mut pollfd = self.socket.create_pollfd(libc::POLLOUT);
                self.socket.poll(&mut pollfd, -1)
            }
            _ => Ok(())
        }
    }
}