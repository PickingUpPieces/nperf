
use std::io::Error;
use std::net::SocketAddrV4;
use std::thread::{self, sleep};
use std::time::Instant;
use crate::util::msghdr_vec::MsghdrVec;
use crate::util::packet_buffer::PacketBuffer;
use io_uring::types::{SubmitArgs, Timespec};
use io_uring::{opcode, squeue, types, CompletionQueue, IoUring, SubmissionQueue};
use log::{debug, error, info, trace, warn};
use io_uring::buf_ring::BufRingSubmissions;

use crate::net::{socket::Socket, MessageHeader, MessageType};
use crate::util::{self, statistic::*, ExchangeFunction, IOModel};
use super::Node;

const INITIAL_POLL_TIMEOUT: i32 = 10000; // in milliseconds
const IN_MEASUREMENT_POLL_TIMEOUT: i32 = 1000; // in milliseconds
const URING_BGROUP: u16 = 0;
const URING_BURST_SIZE: u32 = 256;

pub struct Server {
    packet_buffer: PacketBuffer,
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
        let packet_buffer = PacketBuffer::new(MsghdrVec::new(parameter.packet_buffer_size, parameter.mss, parameter.datagram_size as usize).with_cmsg_buffer());

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
        // Normally, we need to reset the msg_controllen field to the buffer size of all msghdr structs, since the kernel overwrites the value on return.
        // The same applies to the msg_flags field, which is set by the kernel in the msghdr struct.
        // To safe performance, we don't reset the fields, and ignore the msg_flags.
        // The msg_controllen field should be the same for all messages, since it should only contain the GRO enabled control message.

        // if self.parameter.socket_options.gro {
        //     self.packet_buffer.reset_msghdr_fields();
        // }

        match self.exchange_function {
            ExchangeFunction::Normal => self.recv(),
            ExchangeFunction::Msg => self.recvmsg(),
            ExchangeFunction::Mmsg => self.recvmmsg(),
        }
    }

    fn recv(&mut self) -> Result<(), &'static str> {
        // Only one buffer is used, so we can directly access the first element
        let buffer_pointer = self.packet_buffer.get_buffer_pointer_from_index(0).unwrap();

        match self.socket.recv(buffer_pointer) {
            Ok(amount_received_bytes) => {
                let test_id = MessageHeader::get_test_id(buffer_pointer) as usize;
                let mtype = MessageHeader::get_message_type(buffer_pointer);
                debug!("Received packet with test id: {}", test_id);

                self.parse_message_type(mtype, test_id)?;

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

    fn recvmsg(&mut self) -> Result<(), &'static str> {
        // Only one buffer is used, so we can directly access the first element
        let msghdr = self.packet_buffer.get_msghdr_from_index(0).unwrap();

        match self.socket.recvmsg(msghdr) {
            Ok(amount_received_bytes) => {
                let buffer_pointer = self.packet_buffer.get_buffer_pointer_from_index(0).unwrap();
                let test_id = MessageHeader::get_test_id(buffer_pointer) as usize;
                let mtype = MessageHeader::get_message_type(buffer_pointer);
        
                self.parse_message_type(mtype, test_id)?;
        
                let msghdr = self.packet_buffer.get_msghdr_from_index(0).unwrap();
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
        match self.socket.recvmmsg(&mut self.packet_buffer.mmsghdr_vec) {
            Ok(amount_received_mmsghdr) => { 
                if amount_received_mmsghdr == 0 {
                    debug!("No packets received during this recvmmsg call");
                    return Ok(());
                }

                let test_id = MessageHeader::get_test_id(self.packet_buffer.get_buffer_pointer_from_index(0).unwrap()) as usize;
                let mtype = MessageHeader::get_message_type(self.packet_buffer.get_buffer_pointer_from_index(0).unwrap());
                let amount_received_bytes = util::get_total_bytes(&self.packet_buffer.mmsghdr_vec, amount_received_mmsghdr);

                self.parse_message_type(mtype, test_id)?;

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
                // Resize the vector if neeeded, and create a new measurement struct
                if self.measurements.len() <= test_id {
                    self.measurements.resize(test_id + 1, Measurement::new(self.parameter));
                }
                Err("INIT_MESSAGE_RECEIVED")
            },
            MessageType::MEASUREMENT => { 
                let measurement = if let Some(x) = self.measurements.get_mut(test_id) {
                    x
                } else {
                    // No INIT message received before, so we need to resize and create/add a new measurement struct
                    if self.measurements.len() <= test_id {
                        self.measurements.resize(test_id + 1, Measurement::new(self.parameter));
                    }
                    self.measurements.get_mut(test_id).expect("Error getting statistic in measurement message: test id not found")
                };
                // Start measurement timer with receiving of the first MEASUREMENT message
                if !measurement.first_packet_received {
                    info!("{:?}: First packet received from test {}!", thread::current().id(), test_id);
                    measurement.start_time = Instant::now();
                    measurement.first_packet_received = true;
                }
                Ok(())
            },
            MessageType::LAST => {
                info!("{:?}: LAST packet received from test {}!", thread::current().id(), test_id);
                // Not checking for measurement length anymore, since we assume that the thread has received at least one MEASUREMENT message before
                let measurement = self.measurements.get_mut(test_id).expect("Error getting statistic in last message: test id not found");
                let end_time = Instant::now() - std::time::Duration::from_millis(crate::WAIT_CONTROL_MESSAGE); // REMOVE THIS, if you remove the sleep in the client, before sending last message, as well
                measurement.last_packet_received = true;
                measurement.statistic.set_test_duration(measurement.start_time, end_time);
                measurement.statistic.calculate_statistics();
                Err("LAST_MESSAGE_RECEIVED")
            }
        }
    }

    fn io_uring_submit(&mut self, sq: &mut SubmissionQueue, msghdr: &mut libc::msghdr) -> Result<i32, &'static str> {
        let mut submission_count = 0;

        sq.sync(); // Sync sq data structure with io_uring submission queue (Unecessary here, but for debugging purposes)
        debug!("BEGIN io_uring_submit: Current sq len: {}. Dropped messages: {}", sq.len(), sq.dropped());

        // Use the socket file descripter to receive messages
        let fd = self.socket.get_socket_id();

        // TODO: Use multishot recv to receive multiple messages at once: https://docs.rs/io-uring/latest/io_uring/opcode/struct.RecvMsgMulti.html

        for _ in 0..URING_BURST_SIZE {
            // Use io_uring_prep_recvmsg to receive messages: https://docs.rs/io-uring/latest/io_uring/opcode/struct.RecvMsg.html
            let sqe = opcode::RecvMsg::new(types::Fd(fd), msghdr)
            .buf_group(URING_BGROUP) // TODO: Check for parameter
            .build()
            .user_data(self.next_packet_id)
            .flags(squeue::Flags::BUFFER_SELECT);

            unsafe {
                if sq.push(&sqe).is_err() {
                    // TODO: Potentially create either backlog queue or revert packet count to previous, if submitting fails
                    error!("Error pushing io_uring sqe");
                    return Err("IO_URING ERROR")
                }
            }
            submission_count += 1;
        }

        sq.sync(); // Sync sq data structure with io_uring submission queue 
        debug!("END io_uring_submit: Submitted {} io_uring sqe. Current sq len: {}. Dropped messages: {}", submission_count, sq.len(), sq.dropped());
        Ok(submission_count)
    }

    fn io_uring_complete(&mut self, cq: &mut CompletionQueue, bufs: &mut BufRingSubmissions, msghdr: &mut libc::msghdr) -> Result<i32, &'static str> {
        let mut completion_count = 0;

        cq.sync(); // Sync cq data structure with io_uring completion queue
        debug!("BEGIN io_uring_complete: Current cq len: {}. Dropped messages: {}", cq.len(), cq.overflow());

        // Drain completion queue events
        for cqe in cq {
            let amount_received_bytes = cqe.result();
            let token_index = cqe.user_data();
            debug!("Received completion event with user_data: {}, and received bytes: {}", token_index, amount_received_bytes); 

            // Same as in socket.recvmsg function: Check if result is negative, and handle the error
            match amount_received_bytes {
                0 => {
                    warn!("Received empty message");
                    completion_count += 1;
                    continue;
                },
                -105 => { // result is -105, ENOBUFS, no buffer space available
                    warn!("ENOBUFS: No buffer space available, message was truncated");
                    completion_count += 1;
                    continue;
                },
                _ => {
                    if amount_received_bytes < 0 {
                        let errno = Error::last_os_error();
                        match errno.raw_os_error() {
                            // If no messages are available at the socket, the receive calls wait for a message to arrive, unless the socket is nonblocking (see fcntl(2)), in which case the value -1 is returned and the external variable errno is set to EAGAIN or EWOULDBLOCK.
                            // From: https://linux.die.net/man/2/recvmsg
                            Some(libc::EAGAIN) => { return Err("EAGAIN"); },
                            Some(libc::EXIT_SUCCESS) => { break; }, // TODO: This is the error sometimes
                            _ => {
                                error!("Error receiving message: {}", errno);
                                return Err("Failed to receive data!");
                            } 
                        }
                    }
                }
            }

            // Get specific buffer from the buffer ring
            let mut buf = unsafe {
                bufs.get(cqe.flags(), usize::try_from(cqe.result()).unwrap())
            };

            // https://docs.rs/io-uring/latest/io_uring/types/struct.RecvMsgOut.html
            // Helps parsing buffer of multishot recvmsg
            // https://github.com/SUPERCILEX/clipboard-history/blob/95bae326388d7f6f4a63fead5eca4851fd2de1c8/server/src/reactor.rs#L211
            //let msg = io_uring::types::RecvMsgOut::parse(&mut buf, &msghdr).expect("Parsing of RecvMsgOut failed. Didn't allocate large enough buffers");
            //trace!("Received message: {:?}", msg);
            // https://github.com/SUPERCILEX/clipboard-history/blob/95bae326388d7f6f4a63fead5eca4851fd2de1c8/server/src/reactor.rs#L323C21-L326C22
            //if msg.is_control_data_truncated() {
            //    debug!("The control data was truncated");
            //} else if msg.is_payload_truncated() {
            //    debug!("The payload was truncated");
            //} else if msg.is_name_data_truncated() {
            //    // The name data is always truncated, but we don't care about the name data, since we identify the tests with the test_id.
            //    //debug!("The name data was truncated");
            //}

            // Build iovec struct for recvmsg to reuse handle_recvmsg_return code
            let iovec: libc::iovec = libc::iovec {
                iov_base: buf.as_mut_ptr() as *mut libc::c_void,
                iov_len: amount_received_bytes as usize
            };

            let mut msghdr: libc::msghdr = {
                let mut hdr = unsafe { std::mem::zeroed::<libc::msghdr>() };
                hdr.msg_iov = &iovec as *const _ as *mut _;
                hdr.msg_iovlen = 1;
                hdr
            };

            // Parse recvmsg msghdr on return
            // TODO: Should do the same (AND return the same errors) as the normal recvmsg function.
            // TODO: Struct to catch this should be the same as the match block from original recv_messages loop
            // Maybe when using multishot recvmsg, we can add an own io_uring function to recv_messages() and use the same loop
            match self.handle_recvmsg_return(amount_received_bytes, &buf, &mut msghdr) {
                Ok(_) => {},
                Err("INIT_MESSAGE_RECEIVED") => break,
                Err("LAST_MESSAGE_RECEIVED") => {
                    for measurement in self.measurements.iter() {
                        if !measurement.last_packet_received && measurement.first_packet_received {
                            debug!("{:?}: Last message received, but not all measurements are finished!", thread::current().id());
                        } 
                    };
                    info!("{:?}: Last message received and all measurements are finished!", thread::current().id());
                    return Err("LAST_MESSAGE_RECEIVED");
                },
                Err(x) => {
                    error!("Error receiving message! Aborting measurement...");
                    return Err(x);
                }
            }

            // Successful received one message 
            completion_count += 1;
        }
        Ok(completion_count)
    }

    
    fn io_uring_loop(&mut self) -> Result<(), &'static str> {
        let mut ring = IoUring::<io_uring::squeue::Entry>::builder()
        //.setup_coop_taskrun()
        //.setup_single_issuer()
        // .setup_sqpoll(2000) 
        // https://docs.rs/io-uring/latest/io_uring/struct.Builder.html#method.setup_sqpoll_cpu
        // .setup_sqpoll_cpu(0) // CPU to run the SQ poll thread on
        .build(URING_BURST_SIZE * 2).expect("Failed to create io_uring");
        // TODO: Set IORING_FEAT_NODROP flag to handle ring drops

        // TODO: Register provided buffers with io_uring
        let mut buf_ring = ring
        .submitter()
        .register_buf_ring(u16::try_from(URING_BURST_SIZE * 4).unwrap(), URING_BGROUP, (self.packet_buffer.single_packet_buffer_size()) as u32)
        .expect("Creation of BufRing failed.");

        let mut bufs = buf_ring.submissions();
        let (submitter, mut sq, mut cq) = ring.split();
        let mut submission_count: i32 = 0;
        let mut completion_count: i32 = 0;

        // https://github.com/SUPERCILEX/clipboard-history/blob/418b2612f8e62693e42057029df78f6fbf49de3e/server/src/reactor.rs#L206
        // https://github.com/axboe/liburing/blob/cc61897b928e90c4391e0d6390933dbc9088d98f/examples/io_uring-udp.c#L113
        // Only msg_controllen and msg_namelen relevant, when using provided buffers
        let mut msghdr = {
            let mut hdr = unsafe { std::mem::zeroed::<libc::msghdr>() };
            hdr.msg_controllen = 24;
            hdr
        };

        loop {
            // If too many messages are in flight, wait for completions
            if submission_count <= (completion_count + (URING_BURST_SIZE * 3) as i32) {
                submission_count += self.io_uring_submit(&mut sq, &mut msghdr)?;
            }

            // Submit to kernel and wait for 1 completion event or timeout. In case the thread doesn't receive any messages.
            let mut args = SubmitArgs::new();
            let ts = Timespec::new().nsec(100_000_000); 
            args = args.timespec(&ts);
            
            match submitter.submit_with_args(5, &args) {
                Ok(_) => {},
                // If this overflow condition is entered, attempting to submit more IO with fail with the -EBUSY error value, if it can’t flush the overflown events to the CQ ring. 
                // If this happens, the application must reap events from the CQ ring and attempt the submit again.
                // Should ONLY appear when using flag IORING_FEAT_NODROP
                Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => (),
                Err(ref err) if err.raw_os_error() == Some(62) => (), // Timeout error
                Err(err) => {
                    error!("Error submitting io_uring sqe: {}", err);
                    return Err("IO_URING ERROR");
                }
            };

            match self.io_uring_complete(&mut cq, &mut bufs, &mut msghdr) {
                Ok(x) => completion_count += x,
                Err("LAST_MESSAGE_RECEIVED") => {
                    return Ok(());
                },
                Err(x) => {
                    return Err(x);
                }
            };

            bufs.sync(); // Returns used buffers to the buffer ring

            debug!("Submission count: {}, Completion count: {}", submission_count, completion_count);
        }
    }

    fn handle_recvmsg_return(&mut self, amount_received_bytes: i32, buffer_pointer: &[u8], msghdr: &mut libc::msghdr) -> Result<(), &'static str> {
        let test_id = MessageHeader::get_test_id(buffer_pointer) as usize;
        let mtype = MessageHeader::get_message_type(buffer_pointer);

        self.parse_message_type(mtype, test_id)?;

        let statistic = &mut self.measurements.get_mut(test_id).expect("Error getting statistic: test id not found").statistic;
        let absolut_packets_received;
        (self.next_packet_id, absolut_packets_received) = util::process_packet_msghdr(msghdr, amount_received_bytes as usize, self.next_packet_id, statistic);
        statistic.amount_datagrams += absolut_packets_received;
        statistic.amount_data_bytes += amount_received_bytes as usize;
        debug!("Received {} packets and total {} Bytes, and next packet id should be {}", absolut_packets_received, amount_received_bytes, self.next_packet_id);

        Ok(())
    }
}


impl Node for Server { 
    fn run(&mut self, io_model: IOModel) -> Result<Statistic, &'static str> {
        info!("Start server loop...");
        let mut statistic = Statistic::new(self.parameter);

        // Timeout waiting for first message 
        // With communication channel in future, the measure thread is only started if the client starts a measurement. Then timeout can be further reduced to 1-2s.
        let mut pollfd = self.socket.create_pollfd(libc::POLLIN);
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

        if io_model == IOModel::IoUring {
            self.io_uring_loop()?;
        } else {
            'outer: loop {
                match self.recv_messages() {
                    Ok(_) => {},
                    Err("EAGAIN") => {
                        statistic.amount_io_model_syscalls += 1;
                        match self.io_wait(io_model) {
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
                        continue 'outer;
                    },
                    Err(x) => {
                        error!("Error receiving message! Aborting measurement...");
                        return Err(x)
                    }
                }
                statistic.amount_syscalls += 1;
            }
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
