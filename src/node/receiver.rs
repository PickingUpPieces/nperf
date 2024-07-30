use std::net::SocketAddrV4;
use std::os::fd::RawFd;
use std::thread::{self, sleep};
use std::time::Instant;
use log::{debug, error, info, trace, warn};

use io_uring::types::RecvMsgOut;
use crate::io_uring::multishot::IoUringMultishot;
use crate::io_uring::normal::IoUringNormal;
use crate::io_uring::provided_buffer::IoUringProvidedBuffer;
use crate::io_uring::{parse_received_bytes, IoUringOperatingModes, UringMode};
use crate::util::msghdr_vec::MsghdrVec;
use crate::util::packet_buffer::PacketBuffer;
use crate::net::{socket::Socket, MessageHeader, MessageType};
use crate::util::{self, statistic::*, ExchangeFunction, IOModel};
use super::Node;

const INITIAL_POLL_TIMEOUT: i32 = 10000; // in milliseconds
const IN_MEASUREMENT_POLL_TIMEOUT: i32 = 1000; // in milliseconds

pub struct Receiver {
    packet_buffer: PacketBuffer,
    socket: Socket,
    io_uring_sqpoll_fd: Option<RawFd>,
    next_packet_id: u64,
    parameter: Parameter,
    measurements: Vec<Measurement>,
    statistic_interval: StatisticInterval,
    exchange_function: ExchangeFunction
}

impl Receiver {
    pub fn new(sock_address_in: SocketAddrV4, socket: Option<Socket>, io_uring: Option<RawFd>, parameter: Parameter) -> Receiver {
        let socket = if let Some(socket) = socket {
            socket
        } else {
            let mut socket: Socket = Socket::new(parameter.socket_options).expect("Error creating socket");
            socket.bind(sock_address_in).expect("Error binding to local port");
            socket
        };

        info!("Current mode 'receiver' listening on {}:{} with socketID {}", sock_address_in.ip(), sock_address_in.port(), socket.get_socket_id());
        let packet_buffer = PacketBuffer::new(MsghdrVec::new(parameter.packet_buffer_size, parameter.mss, parameter.datagram_size as usize).with_cmsg_buffer());

        Receiver {
            packet_buffer,
            socket,
            io_uring_sqpoll_fd: io_uring,
            next_packet_id: 0,
            parameter: parameter.clone(),
            measurements: Vec::new(),
            statistic_interval: StatisticInterval::new(Instant::now(), parameter.output_interval, parameter.test_runtime_length),
            exchange_function: parameter.exchange_function
        }
    }

    fn recv_messages(&mut self) -> Result<(), &'static str> {
        // The msg_controllen field should be the same for all messages, since it should only contain the GRO enabled control message.
        // It is only reset after the first message, since the first message is the INIT message, which doesn't contain any control messages.

        if self.parameter.socket_options.gro && self.next_packet_id == 0 {
            self.packet_buffer.reset_msghdr_fields();
        }

        match self.exchange_function {
            ExchangeFunction::Normal => self.recv(),
            ExchangeFunction::Msg => self.recvmsg(),
            ExchangeFunction::Mmsg => self.recvmmsg(),
        }
    }

    #[inline(always)]
    fn recv(&mut self) -> Result<(), &'static str> {
        // Only one buffer is used, so we can directly access the first element
        let buffer_pointer = self.packet_buffer.get_buffer_pointer_from_index(0).unwrap();

        match self.socket.recv(buffer_pointer) {
            Ok(amount_received_bytes) => {
                let test_id = MessageHeader::get_test_id(buffer_pointer) as usize;
                let mtype = MessageHeader::get_message_type(buffer_pointer);
                debug!("Received packet with test id: {}", test_id);

                Self::parse_message_type(mtype, test_id, &mut self.measurements, &self.parameter)?;

                let statistic = &mut self.measurements.get_mut(test_id).expect("Error getting statistic: test id not found").statistic;
                let datagram_size = self.packet_buffer.datagram_size();
                let amount_received_packets = util::process_packet_buffer(self.packet_buffer.get_buffer_pointer_from_index(0).unwrap(), datagram_size, self.next_packet_id, statistic);
                self.next_packet_id += amount_received_packets;
                statistic.amount_datagrams += amount_received_packets;
                statistic.amount_data_bytes += amount_received_bytes;
                Ok(())
            },
            Err(x) => Err(x)
        }
    }

    #[inline(always)]
    fn recvmsg(&mut self) -> Result<(), &'static str> {
        // Only one buffer is used, so we can directly access the first element
        let msghdr = self.packet_buffer.get_msghdr_from_index(0).unwrap();

        match self.socket.recvmsg(msghdr) {
            Ok(amount_received_bytes) => {
                let buffer_pointer = self.packet_buffer.get_buffer_pointer_from_index(0).unwrap();
                let test_id = MessageHeader::get_test_id(buffer_pointer) as usize;
                let mtype = MessageHeader::get_message_type(buffer_pointer);
        
                Self::parse_message_type(mtype, test_id, &mut self.measurements, &self.parameter)?;
        
                let msghdr = self.packet_buffer.get_msghdr_from_index(0).unwrap();
                let statistic = &mut self.measurements.get_mut(test_id).expect("Error getting statistic: test id not found").statistic;
                let absolut_packets_received;
                (self.next_packet_id, absolut_packets_received) = util::process_packet_msghdr(msghdr, amount_received_bytes, self.next_packet_id, statistic);
                statistic.amount_datagrams += absolut_packets_received;
                statistic.amount_data_bytes += amount_received_bytes;

                // Reset msg_flags and msg_controllen fields
                if self.parameter.socket_options.gro {
                    msghdr.msg_flags = 0;
                    msghdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
                }

                debug!("Received {} packets and total {} Bytes, and next packet id should be {}", absolut_packets_received, amount_received_bytes, self.next_packet_id);
                Ok(())
            },
            Err(x) => Err(x)
        }
    }

    #[inline(always)]
    fn recvmmsg(&mut self) -> Result<(), &'static str> {
        match self.socket.recvmmsg(&mut self.packet_buffer.mmsghdr_vec) {
            Ok(amount_received_mmsghdr) => { 
                if amount_received_mmsghdr == 0 {
                    debug!("No packets received during this recvmmsg call");
                    return Ok(());
                }

                let test_id = MessageHeader::get_test_id(self.packet_buffer.get_buffer_pointer_from_index(0).unwrap()) as usize;
                let mtype = MessageHeader::get_message_type(self.packet_buffer.get_buffer_pointer_from_index(0).unwrap());
                let amount_received_bytes = util::get_total_bytes(&self.packet_buffer.mmsghdr_vec, amount_received_mmsghdr);

                Self::parse_message_type(mtype, test_id, &mut self.measurements, &self.parameter)?;

                let statistic = &mut self.measurements.get_mut(test_id).expect("Error getting statistic: test id not found").statistic;
                let mut absolut_datagrams_received = 0;

                // Check and calculate the amount of received packets and bytes
                for (index, mmsghdr) in self.packet_buffer.mmsghdr_vec.iter_mut().enumerate() {
                    if index >= amount_received_mmsghdr {
                        break;
                    }
                    let msghdr = &mut mmsghdr.msg_hdr;
                    let msghdr_bytes = mmsghdr.msg_len as usize;

                    let datagrams_received;
                    (self.next_packet_id, datagrams_received) = util::process_packet_msghdr(msghdr, msghdr_bytes, self.next_packet_id, statistic);
                    absolut_datagrams_received += datagrams_received;
                                    
                    if self.parameter.socket_options.gro {
                        msghdr.msg_flags = 0;
                        msghdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
                    }
                }

                statistic.amount_datagrams += absolut_datagrams_received;
                statistic.amount_data_bytes += amount_received_bytes;
                trace!("Sent {} msg_hdr to remote host", amount_received_mmsghdr);
                Ok(())
            },
            Err(x) => Err(x)
        }
    }

    fn parse_message_type(mtype: MessageType, test_id: usize, measurements: &mut Vec<Measurement>, parameter: &Parameter) -> Result<(), &'static str> {
        if test_id >= crate::MAX_TEST_ID {
            error!("Received test id is greater than the maximum test id: {} > {}!", test_id, crate::MAX_TEST_ID);
            return Err("Received test id is greater than the maximum test id")
        }
        match mtype {
            MessageType::INIT => {
                info!("{:?}: INIT packet received from test {}!", thread::current().id(), test_id);
                // Resize the vector if neeeded, and create a new measurement struct
                if measurements.len() <= test_id {
                    measurements.resize(test_id + 1, Measurement::new(parameter.clone()));
                }
                Err("INIT_MESSAGE_RECEIVED")
            },
            MessageType::MEASUREMENT => { 
                let measurement = if let Some(x) = measurements.get_mut(test_id) {
                    x
                } else {
                    // No INIT message received before, so we need to resize and create/add a new measurement struct
                    if measurements.len() <= test_id {
                        measurements.resize(test_id + 1, Measurement::new(parameter.clone()));
                    }
                    measurements.get_mut(test_id).expect("Error getting statistic in measurement message: test id not found")
                };
                // Start measurement timer with receiving of the first MEASUREMENT message
                if !measurement.first_packet_received {
                    info!("{:?}: First packet received from test {}!", thread::current().id(), test_id);
                    let start_time = Statistic::get_unix_timestamp();
                    measurement.start_time = start_time;
                    measurement.statistic.set_start_timestamp(Some(start_time));
                    measurement.first_packet_received = true;
                }
                Ok(())
            },
            MessageType::LAST => {
                info!("{:?}: LAST packet received from test {}!", thread::current().id(), test_id);
                // Not checking for measurement length anymore, since we assume that the thread has received at least one MEASUREMENT message before
                let measurement = measurements.get_mut(test_id).expect("Error getting statistic in last message: test id not found");
                // Convert to from milliseconds to seconds
                let end_time = Statistic::get_unix_timestamp() - (crate::WAIT_CONTROL_MESSAGE as f64 / 1000.0); // REMOVE THIS, if you remove the sleep in the sender, before sending last message, as well
                measurement.last_packet_received = true;
                measurement.statistic.set_test_duration(Some(measurement.start_time), Some(end_time));
                measurement.statistic.calculate_statistics();
                measurement.statistic.set_end_timestamp();
                Err("LAST_MESSAGE_RECEIVED")
            }
        }
    }



    fn io_uring_complete_normal(&mut self, io_uring_instance: &mut IoUringNormal) -> Result<u32, &'static str> {
        let mut completion_count = 0;
        // We do not need to cq.sync the completion queue, since this is done automatically when getting/dropping the cq object
        let cq = io_uring_instance.get_cq();
        // Pool to store the buffer indexes, which are used in the completion queue to return them later
        let mut index_pool: Vec<usize> = Vec::with_capacity(cq.len());

        debug!("BEGIN io_uring_complete: Current cq len: {}/{}", cq.len(), cq.capacity());

        if cq.overflow() > 0 {
            warn!("NO_DROP feature not available: Dropped messages in completion queue: {}", cq.overflow());
        }

        // Drain completion queue events
        for cqe in cq {
            let amount_received_bytes = cqe.result();
            let user_data = cqe.user_data();
            debug!("Received completion event with user_data: {}, and received bytes: {}", user_data, amount_received_bytes); 

            completion_count += parse_received_bytes(amount_received_bytes)?;

            match self.handle_recvmsg_return(amount_received_bytes, None, user_data) {
                Ok(_) => {},
                Err("INIT_MESSAGE_RECEIVED") => { // Checking for INIT message, and returning the buffer index to the buffer ring
                    index_pool.push(user_data as usize);
                    continue;
                },
                Err(x) => return Err(x)
            };

            index_pool.push(user_data as usize);
        }

        // Returns used buffers to the buffer ring.
        self.packet_buffer.return_buffer_index(index_pool);

        debug!("END io_uring_complete: Completed {} io_uring cqe", completion_count);
        Ok(completion_count)
    }


    fn io_uring_complete_provided_buffers(&mut self, io_uring_instance: &mut IoUringProvidedBuffer, statistic: &mut Statistic) -> Result<u32, &'static str> {
        let mut completion_count = 0;
        let (buf_ring, cq) = io_uring_instance.get_bufs_and_cq();
        let mut bufs = buf_ring.submissions();

        debug!("BEGIN io_uring_complete: Current cq len: {}/{}", cq.len(), cq.capacity());

        if cq.overflow() > 0 {
            warn!("NO_DROP feature not available: Dropped messages in completion queue: {}", cq.overflow());
        }

        // Drain completion queue events
        for cqe in cq {
            let amount_received_bytes = cqe.result();
            let user_data = cqe.user_data();
            debug!("Received completion event with user_data: {}, and received bytes: {}", user_data, amount_received_bytes); 

            match parse_received_bytes(amount_received_bytes) {
                Ok(0) => { // On ENOBUFS, we need to continue with the next cqe
                    // FIXME: Add out of buffer statistic counter
                    statistic.uring_out_of_buffers += 1;
                    completion_count += 1;
                    continue;
                },
                Ok(i) => completion_count += i,
                Err(x) => return Err(x)
            }

            // Create a msghdr from the provided buffer to better parse the received message
            let mut buf = unsafe {
                bufs.get(cqe.flags(), usize::try_from(amount_received_bytes).unwrap())
            };

            // Build iovec struct for recvmsg to reuse handle_recvmsg_return code
            let iovec = libc::iovec {
                iov_base: buf.as_mut_ptr() as *mut libc::c_void,
                iov_len: amount_received_bytes as usize
            };

            let mut msghdr = {
                let mut hdr = unsafe { std::mem::zeroed::<libc::msghdr>() };
                hdr.msg_iov = &iovec as *const _ as *mut _;
                hdr.msg_iovlen = 1;
                hdr
            };

            self.handle_recvmsg_return(amount_received_bytes, Some(&mut msghdr), user_data)?; 
        }

        debug!("END io_uring_complete: Completed {} io_uring cqe", completion_count);
        Ok(completion_count)
    }


    fn io_uring_complete_multishot(&mut self,  io_uring_instance: &mut IoUringMultishot) -> Result<bool, &'static str> {
        let mut multishot_armed = true;
        let msghdr = &io_uring_instance.get_msghdr();
        let (buf_ring, cq) = io_uring_instance.get_bufs_and_cq();
        let mut bufs = buf_ring.submissions();

        debug!("BEGIN io_uring_complete: Current cq len: {}/{}", cq.len(), cq.capacity());

        if cq.overflow() > 0 {
            warn!("NO_DROP feature not available: Dropped messages in completion queue: {}", cq.overflow());
        }

        for cqe in cq {
            let amount_received_bytes = cqe.result();
            debug!("Received completion event with bytes: {}", amount_received_bytes); 

            if parse_received_bytes(amount_received_bytes)? == 0 {
                // If one multishot operation failed, make sure with logical AND that is stays false
                // In provided buffers, we continue when we receive error ENOBUFS. In multishot we've returned with Ok(check_flags). Try to continue with the next cqe in multishot as well.
                multishot_armed &= crate::io_uring::check_multishot_status(cqe.flags()); 
                continue; 
            };

            multishot_armed &= crate::io_uring::check_multishot_status(cqe.flags()); 

            // Get specific buffer from the buffer ring
            let buf = unsafe {
                bufs.get(cqe.flags(), usize::try_from(amount_received_bytes).unwrap())
            };

            // Helps parsing buffer of multishot recvmsg
            // https://docs.rs/io-uring/latest/io_uring/types/struct.RecvMsgOut.html
            // https://github.com/SUPERCILEX/clipboard-history/blob/95bae326388d7f6f4a63fead5eca4851fd2de1c8/receiver/src/reactor.rs#L211
            let msg = RecvMsgOut::parse(&buf, msghdr).expect("Parsing of RecvMsgOut failed. Didn't allocate large enough buffers");
            trace!("Received message: {:?}", msg);

            // Check if any data is truncated
            if msg.is_control_data_truncated() {
                debug!("The control data was truncated");
            } else if msg.is_payload_truncated() {
                debug!("The payload was truncated");
            } else if msg.is_name_data_truncated() {
                debug!("The name data was truncated");
            }

            // Create a msghdr from the provided buffer to better parse the received message
            let iovec: libc::iovec = libc::iovec {
                iov_base: msg.payload_data().as_ptr() as *mut libc::c_void,
                iov_len: (amount_received_bytes - crate::URING_ADDITIONAL_BUFFER_LENGTH) as usize
            };

            let mut msghdr: libc::msghdr = {
                let mut hdr = unsafe { std::mem::zeroed::<libc::msghdr>() };
                hdr.msg_iov = &iovec as *const _ as *mut _;
                hdr.msg_iovlen = 1;
                hdr.msg_control = msg.control_data().as_ptr() as *mut libc::c_void;
                hdr.msg_controllen = msg.control_data().len();
                hdr
            };
 
            self.handle_recvmsg_return(amount_received_bytes - crate::URING_ADDITIONAL_BUFFER_LENGTH,  Some(&mut msghdr), 0)?;
        }

        debug!("END io_uring_complete: Multishot is still armed: {}", multishot_armed);
        Ok(multishot_armed)
    }

    fn handle_recvmsg_return(&mut self, amount_received_bytes: i32,  msghdr: Option<&mut libc::msghdr>, msghdr_index: u64) -> Result<(), &'static str> {
        let msghdr = match msghdr {
            Some(msghdr) => msghdr,
            None => self.packet_buffer.get_msghdr_from_index(msghdr_index as usize).unwrap()
        };

        let libc::iovec { iov_base, iov_len } = unsafe {*msghdr.msg_iov};
        let buffer_pointer = unsafe {
            // Get buffer from iov_base with type &[u8]
            std::slice::from_raw_parts(iov_base as *const u8, iov_len )
        };
        
        let test_id = MessageHeader::get_test_id(buffer_pointer) as usize;
        let mtype = MessageHeader::get_message_type(buffer_pointer);

        Self::parse_message_type(mtype, test_id, &mut self.measurements, &self.parameter)?;

        let msghdr = match self.parameter.uring_parameter.uring_mode {
            UringMode::Normal => self.packet_buffer.get_msghdr_from_index(msghdr_index as usize).unwrap(),
            _ => msghdr
        };

        let statistic = &mut self.measurements.get_mut(test_id).expect("Error getting statistic: test id not found").statistic;
        let absolut_packets_received;
        (self.next_packet_id, absolut_packets_received) = util::process_packet_msghdr(msghdr, amount_received_bytes as usize, self.next_packet_id, statistic);
        statistic.amount_datagrams += absolut_packets_received;
        statistic.amount_data_bytes += amount_received_bytes as usize;

        // Reset msg_flags and msg_controllen fields
        if self.parameter.socket_options.gro {
            msghdr.msg_flags = 0;
            msghdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
        }

        debug!("Received {} packets and total {} Bytes, and next packet id should be {}", absolut_packets_received, amount_received_bytes, self.next_packet_id);
        Ok(())
    }


    fn io_uring_loop(&mut self) -> Result<Statistic, &'static str> {
        let socket_fd = self.socket.get_socket_id();
        let mut statistic = Statistic::new(self.parameter.clone());
        let mut amount_inflight = 0;

        match self.parameter.uring_parameter.uring_mode {
            UringMode::Multishot => {
                let mut io_uring_instance = crate::io_uring::multishot::IoUringMultishot::new(self.parameter.clone(), self.io_uring_sqpoll_fd)?;
                // Indicator if multishot request is still armed
                let mut armed = false;

                loop {
                    statistic.amount_io_model_calls += 1;
                    io_uring_instance.fill_sq_and_submit(armed, socket_fd)?;

                    // Check if the time elapsed since the last send operation is greater than or equal to self.parameters.interval seconds
                    if self.statistic_interval.output_interval != 0.0 && self.statistic_interval.last_send_instant.elapsed().as_secs_f64() >= self.statistic_interval.output_interval {
                        let mut statistic_new = self.measurements.iter().fold(statistic.clone(), |acc: Statistic, measurement| acc + measurement.statistic.clone());
                        statistic_new = statistic_new + io_uring_instance.get_statistic();
                        self.statistic_interval.calculate_interval(statistic_new);

                        // Reset measurements statistics
                        for measurement in &mut self.measurements {
                            measurement.statistic = Statistic::new(self.parameter.clone()); 
                        }
                        statistic = Statistic::new(self.parameter.clone());
                        io_uring_instance.reset_statistic(self.parameter.clone());
                    }

                    match self.io_uring_complete_multishot(&mut io_uring_instance) {
                        Ok(multishot_armed) => {
                            if !multishot_armed {
                                statistic.uring_canceled_multishot += 1;
                            }
                            armed = multishot_armed
                        },
                        Err("INIT_MESSAGE_RECEIVED") => {},
                        Err("LAST_MESSAGE_RECEIVED") => {
                            if self.all_measurements_finished() { return Ok(statistic + io_uring_instance.get_statistic()) }
                        },
                        Err("EAGAIN") => {
                            statistic.amount_eagain += 1;
                        },
                        Err(x) => {
                            error!("Error completing io_uring sqe: {}", x);
                            return Err(x);
                        }
                    };
                }
            },
            UringMode::ProvidedBuffer => {
                let mut io_uring_instance: IoUringProvidedBuffer = crate::io_uring::provided_buffer::IoUringProvidedBuffer::new(self.parameter.clone(), self.io_uring_sqpoll_fd)?;

                loop {
                    if let Some(ref mut array) = statistic.uring_inflight_utilization {
                        array[amount_inflight as usize] += 1;
                    }
                    statistic.amount_io_model_calls += 1;

                    // Check if the time elapsed since the last send operation is greater than or equal to self.parameters.interval seconds
                    if self.statistic_interval.output_interval != 0.0 && self.statistic_interval.last_send_instant.elapsed().as_secs_f64() >= self.statistic_interval.output_interval {
                        let mut statistic_new = self.measurements.iter().fold(statistic.clone(), |acc: Statistic, measurement| acc + measurement.statistic.clone());
                        statistic_new = statistic_new + io_uring_instance.get_statistic();
                        self.statistic_interval.calculate_interval(statistic_new);

                        // Reset measurements statistics
                        for measurement in &mut self.measurements {
                            measurement.statistic = Statistic::new(self.parameter.clone()); 
                        }
                        statistic = Statistic::new(self.parameter.clone());
                        io_uring_instance.reset_statistic(self.parameter.clone());
                    }

                    amount_inflight += io_uring_instance.fill_sq_and_submit(amount_inflight, socket_fd)?;

                    match self.io_uring_complete_provided_buffers(&mut io_uring_instance, &mut statistic) {
                        Ok(completed) => {
                            amount_inflight -= completed
                        },
                        Err("INIT_MESSAGE_RECEIVED") => {},
                        Err("LAST_MESSAGE_RECEIVED") => {
                            if self.all_measurements_finished() { return Ok(statistic + io_uring_instance.get_statistic()) }
                        },
                        Err("EAGAIN") => {
                            statistic.amount_eagain += 1;
                        },
                        Err(x) => {
                            error!("Error completing io_uring sqe: {}", x);
                            return Err(x);
                        }
                    };
                }
            },
            UringMode::Normal => {
                let mut io_uring_instance = crate::io_uring::normal::IoUringNormal::new(self.parameter.clone(), self.io_uring_sqpoll_fd)?;

                loop {
                    if let Some(ref mut array) = statistic.uring_inflight_utilization {
                        array[amount_inflight as usize] += 1;
                    }
                    statistic.amount_io_model_calls += 1;

                    // Check if the time elapsed since the last send operation is greater than or equal to self.parameters.interval seconds
                    if self.statistic_interval.output_interval != 0.0 && self.statistic_interval.last_send_instant.elapsed().as_secs_f64() >= self.statistic_interval.output_interval {
                        let mut statistic_new = self.measurements.iter().fold(statistic.clone(), |acc: Statistic, measurement| acc + measurement.statistic.clone());
                        statistic_new = statistic_new + io_uring_instance.get_statistic();
                        self.statistic_interval.calculate_interval(statistic_new);

                        // Reset measurements statistics
                        for measurement in &mut self.measurements {
                            measurement.statistic = Statistic::new(self.parameter.clone()); 
                        }
                        statistic = Statistic::new(self.parameter.clone());
                        io_uring_instance.reset_statistic(self.parameter.clone());
                    }

                    amount_inflight += io_uring_instance.fill_sq_and_submit(amount_inflight, &mut self.packet_buffer, socket_fd)?;

                    match self.io_uring_complete_normal(&mut io_uring_instance) {
                        Ok(completed) => {
                            amount_inflight -= completed
                        },
                        Err("INIT_MESSAGE_RECEIVED") => {},
                        Err("LAST_MESSAGE_RECEIVED") => {
                            if self.all_measurements_finished() { return Ok(statistic + io_uring_instance.get_statistic()) } 
                        },
                        Err("EAGAIN") => {
                            statistic.amount_eagain += 1;
                        },
                        Err(x) => {
                            error!("Error completing io_uring sqe: {}", x);
                            return Err(x);
                        }
                    };
                }
            },
            _ => {
                error!("Invalid io_uring mode selected for receiver!");
                Err("Invalid io_uring mode selected for receiver!")
            }
        }
    }

    fn all_measurements_finished(&self) -> bool {
        for measurement in self.measurements.iter() {
            if !measurement.last_packet_received && measurement.first_packet_received {
                debug!("{:?}: Last message received, but not all measurements are finished!", thread::current().id());
                return false;
            }
        }
        info!("{:?}: Last message received and all measurements are finished!", thread::current().id());
        true
    }

}


impl Node for Receiver { 
    fn run(&mut self, io_model: IOModel) -> Result<(Statistic, Vec<Statistic>), &'static str> {
        info!("Start receiver loop...");
        let mut statistic = Statistic::new(self.parameter.clone());

        // Timeout waiting for first message 
        // With communication channel in future, the measure thread is only started if the sender starts a measurement. Then timeout can be further reduced to 1-2s.
        let mut pollfd = self.socket.create_pollfd(libc::POLLIN);
        match self.socket.poll(&mut pollfd, INITIAL_POLL_TIMEOUT) {
            Ok(_) => {},
            Err("TIMEOUT") => {
                // If port sharding is used, not every receiver thread gets packets due to the load balancing of REUSEPORT.
                // To avoid that the thread waits forever, we need to return here.
                warn!("{:?}: Timeout waiting for sender to send first packet!", thread::current().id());
                return Ok((statistic, Vec::new()));
            },
            Err(x) => {
                return Err(x);
            }
        };
        statistic.start_timestamp = self.statistic_interval.last_send_timestamp;
        self.statistic_interval.last_send_instant = Instant::now() + std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE);
        self.statistic_interval.last_send_timestamp = Statistic::get_unix_timestamp() + (crate::WAIT_CONTROL_MESSAGE as f64 / 1000.0);

        if io_model == IOModel::IoUring {
            statistic = self.io_uring_loop()?;
        } else {
            loop {
                statistic.amount_syscalls += 1;

                // Check if the time elapsed since the last send operation is greater than or equal to self.parameters.interval seconds
                if self.statistic_interval.output_interval != 0.0 && self.statistic_interval.last_send_instant.elapsed().as_secs_f64() >= self.statistic_interval.output_interval {
                    let statistic_new = self.measurements.iter().fold(statistic.clone(), |acc: Statistic, measurement| acc + measurement.statistic.clone());
                    self.statistic_interval.calculate_interval(statistic_new);
                    // Reset measurements statistics
                    for measurement in &mut self.measurements {
                        measurement.statistic = Statistic::new(self.parameter.clone()); 
                    }
                    statistic = Statistic::new(self.parameter.clone());
                }

                match self.recv_messages() {
                    Ok(_) => {},
                    Err("EAGAIN") => {
                        statistic.amount_io_model_calls += 1;
                        statistic.amount_eagain += 1;
                        match self.io_wait(io_model) {
                            Ok(_) => {},
                            Err("TIMEOUT") => {
                                // If port sharing is used, or single connection not every thread receives the LAST message. 
                                // To avoid that the thread waits forever, we need to return here.
                                warn!("{:?}: Timeout waiting for a subsequent packet from the sender!", thread::current().id());
                                break;
                            },
                            Err(x) => {
                                return Err(x);
                            }
                        }
                    },
                    Err("INIT_MESSAGE_RECEIVED") => {},
                    Err("LAST_MESSAGE_RECEIVED") => {
                        if self.all_measurements_finished() { break }
                    },
                    Err(x) => {
                        error!("Error receiving message! Aborting measurement...");
                        return Err(x)
                    }
                }
            }
        }

        if self.parameter.multiplex_port_receiver != MultiplexPort::Sharing {
            // If a thread finishes (closes the socket) before the others, the hash mapping of SO_REUSEPORT changes. 
            // Then all threads would receive packets from other connections (test_ids).
            // Therefore, we need to wait a bit, until a thread closes its socket.
            if self.parameter.multiplex_port_receiver == MultiplexPort::Sharding {
                sleep(std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE * 2));
            }
            self.socket.close()?;
        }

        debug!("{:?}: Finished receiving data from remote host", thread::current().id());

        let mut final_statistic = Statistic::new(self.parameter.clone());
        if self.statistic_interval.statistics.is_empty() {
            final_statistic = self.measurements.iter().fold(statistic, |acc: Statistic, measurement| acc + measurement.statistic.clone());
            final_statistic.set_test_duration(Some(self.statistic_interval.last_send_timestamp), Some(Statistic::get_unix_timestamp() - (crate::WAIT_CONTROL_MESSAGE as f64 / 1000.0)));
        } else {
            for statistic in self.statistic_interval.statistics.iter() {
                final_statistic = final_statistic + statistic.clone();
            }
        }

        final_statistic.set_test_duration(None, None);
        final_statistic.calculate_statistics();

        Ok((final_statistic, self.statistic_interval.statistics.clone()))
    }

    fn io_wait(&mut self, io_model: IOModel) -> Result<(), &'static str> {
        // Normally we would need to iterate over FDs and check which socket is ready
        // Since we only have one socket, we directly call recv_messages after io_wait returns
        match io_model {
            IOModel::Select => {
                let mut read_fds: libc::fd_set = unsafe { self.socket.create_fdset() };
                self.socket.select(Some(&mut read_fds), None, IN_MEASUREMENT_POLL_TIMEOUT)

            },
            IOModel::Poll => {
                let mut pollfd = self.socket.create_pollfd(libc::POLLIN);
                self.socket.poll(&mut pollfd, IN_MEASUREMENT_POLL_TIMEOUT)
            },
            _ => Ok(())
        }
    }
}
