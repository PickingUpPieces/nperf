use io_uring::{buf_ring::BufRing, cqueue::Entry, opcode, squeue, types, CompletionQueue, IoUring};
use libc::msghdr;
use log::{debug, trace, warn};
use std::os::{fd::RawFd, unix::io::AsRawFd};

use crate::{util::{packet_buffer::PacketBuffer, statistic::{Parameter, UringParameter}}, Statistic};
use super::UringSqFillingMode;
pub struct IoUringNormal {
    ring: IoUring,
    buf_ring: BufRing,
    parameter: UringParameter,
    msghdr: msghdr, // TODO: Move to provided buffers
    statistic: Statistic
}

impl IoUringNormal {
    pub fn new(parameter: Parameter, io_uring_fd: Option<RawFd>) -> Result<Self, &'static str> {

        let ring = super::create_ring(parameter.uring_parameter, io_uring_fd)?;
        let buf_ring = super::create_buf_ring(&mut ring.submitter(), parameter.uring_parameter.buffer_size as u16, parameter.mss);

        // TODO: Can be moved to provided buffer
        // https://github.com/SUPERCILEX/clipboard-history/blob/418b2612f8e62693e42057029df78f6fbf49de3e/server/src/reactor.rs#L206
        // https://github.com/axboe/liburing/blob/cc61897b928e90c4391e0d6390933dbc9088d98f/examples/io_uring-udp.c#L113
        // Generic msghdr: msg_controllen and msg_namelen relevant, when using provided buffers
        let msghdr = {
            let mut hdr = unsafe { std::mem::zeroed::<libc::msghdr>() };
            hdr.msg_controllen = 24;
            hdr
        };
        
        Ok(IoUringNormal {
            ring,
            buf_ring,
            parameter: parameter.uring_parameter,
            msghdr,
            statistic: Statistic::new(parameter)
        })
    }

    pub fn submit(&mut self, amount_recvmsg: u32, packet_buffer: &mut PacketBuffer, socket_fd: i32) -> Result<u32, &'static str> {
        let mut submission_count = 0;
        let mut sq = self.ring.submission();

        debug!("BEGIN io_uring_submit: Current sq len: {}. Dropped messages: {}", sq.len(), sq.dropped());

        for i in 0..amount_recvmsg {
            let mut packet_buffer_index = 0;
            // Create OPCODE for receiving message
            let sqe = if self.parameter.provided_buffer {
                opcode::RecvMsg::new(types::Fd(socket_fd), &mut self.msghdr)
                .buf_group(crate::URING_BUFFER_GROUP) 
                .build()
                .flags(squeue::Flags::BUFFER_SELECT)
            } else {
                packet_buffer_index = match packet_buffer.get_buffer_index() {
                    Some(index) => {
                        trace!("Message number {}/{}: Used buffer index {}", i, amount_recvmsg, index);
                        index
                    },
                    None => {
                        warn!("No buffers left in packet_buffer");
                        break;
                    }
                };
                // Use io_uring_prep_recvmsg to receive messages: https://docs.rs/io-uring/latest/io_uring/opcode/struct.RecvMsg.html
                opcode::RecvMsg::new(types::Fd(socket_fd), packet_buffer.get_msghdr_from_index(packet_buffer_index)?)
                .build()
                .user_data(packet_buffer_index as u64)
            };

            match unsafe { sq.push(&sqe) } {
                Ok(_) => submission_count += 1,
                Err(err) => {
                    // When using submission queue polling, it can happen that the reported queue length is not the same as the actual queue length.
                    // TODO: Potentially create either backlog queue or revert packet count to previous, if submitting fails
                    warn!("Error pushing io_uring sqe: {}. Stopping submit() after submitting {} entries", err, submission_count);
                    if !self.parameter.provided_buffer {
                        packet_buffer.return_buffer_index(vec![packet_buffer_index]);
                    }
                    break;
                }
            };
        }

        debug!("END io_uring_submit: Submitted {} io_uring sqe. Current sq len: {}. Dropped messages: {}", submission_count, sq.len(), sq.dropped());
        Ok(submission_count)
    }


    pub fn fill_sq_and_submit(&mut self, amount_inflight: u32, packet_buffer: &mut PacketBuffer, socket_fd: i32) -> Result<u32, &'static str> {
        let mut amount_new_requests = 0;

        let min_complete = match super::calc_sq_fill_mode(amount_inflight, self.parameter, &mut self.ring) {
            (0,0) => return Ok(0),
            (to_submit, min_complete) => {
                amount_new_requests += self.submit(to_submit, packet_buffer, socket_fd)?;
                min_complete
            }
        };

        // Utilization of the submission queue
        self.statistic.uring_sq_utilization[self.ring.submission().len()] += 1;
        super::io_uring_enter(&mut self.ring.submitter(), crate::URING_ENTER_TIMEOUT, min_complete)?;

        // Utilization of the completion queue
        self.statistic.uring_cq_utilization[self.ring.completion().len()] += 1;
        debug!("Added {} new requests to submission queue. Current inflight: {}", amount_new_requests, amount_inflight + amount_new_requests);

        Ok(amount_new_requests)
    }

    pub fn get_cq(&mut self) -> CompletionQueue<'_, Entry> {
        self.ring.completion()
    }

    pub fn get_bufs_and_cq(&mut self) -> (&mut BufRing, CompletionQueue<'_, Entry>) {
        (&mut self.buf_ring, self.ring.completion())
    }

    pub fn get_statistic(&self) -> Statistic {
        self.statistic.clone()
    }

    pub fn get_raw_fd(&self) -> i32 {
        self.ring.as_raw_fd()
    }
}