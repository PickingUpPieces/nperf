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
        let uring_burst_size = self.parameter.burst_size;
        let uring_buffer_size = self.parameter.buffer_size;
        let mut amount_new_requests = 0;

        // Check if there are not enough free buffers -> Either wait for completion events or reap them
        if amount_inflight > uring_buffer_size - uring_burst_size {
            if self.ring.completion().is_empty() {
                // No buffers left and cq is empty -> We need to do some work/wait for CQEs
                let min_complete = if uring_burst_size == 0 {
                    self.parameter.ring_size / crate::URING_BURST_SIZE_DIVIDEND // Default burst size
                }  else {
                    uring_burst_size
                } as usize;
            
                super::io_uring_enter(&mut self.ring.submitter(), crate::URING_ENTER_TIMEOUT, min_complete)?;
            }
            // If no buffers left, but CQE events in CQ, we don't want to call io_uring_enter -> Fall through
        } else {
            // There are enough buffers left to fill up the submission queue
            match self.parameter.sq_filling_mode {
                UringSqFillingMode::Syscall => {
                    // Check if the submission queue is max filled with the burst size
                    if amount_inflight < uring_burst_size {
                        amount_new_requests += self.submit(uring_burst_size, packet_buffer, socket_fd)?;
                    }
                    // If there are currently more entries inflight than the burst size, we don't want to submit more entries
                    // Fall through to the completion queue handling
                },
                UringSqFillingMode::Topup => {
                    // Fill up the submission queue to the maximum
                    let sq_entries_left = {
                        let sq = self.ring.submission();
                        sq.capacity() - sq.len()
                    } as u32;
                    let buffers_left = uring_buffer_size - amount_inflight;
                    // Check if enough buffers are left to fill up the submission queue, otherwise only fill up the remaining buffers
                    if buffers_left < sq_entries_left {
                        amount_new_requests += self.submit(buffers_left, packet_buffer, socket_fd)?;
                    } else {
                        amount_new_requests += self.submit(sq_entries_left, packet_buffer, socket_fd)?;
                    }
                }
            };

            // Utilization of the submission queue
            self.statistic.uring_sq_utilization[self.ring.submission().len()] += 1;

            // SQ_POLL: Only reason to call io_uring_enter is to wake up SQ_POLL thread.
		    //          Due to the library we're using, the library function will only trigger the syscall io_uring_enter, if the sq_poll thread is asleep.
            //          If min_complete > 0, io_uring_enter syscall is triggered, so for SQ_POLL we don't want this normally.
		    //          If other task_work is implemented, we need to force this probably.
            let min_complete = if self.parameter.sqpoll { 0 } else { uring_burst_size } as usize;
            super::io_uring_enter(&mut self.ring.submitter(), crate::URING_ENTER_TIMEOUT, min_complete)?;
        }

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