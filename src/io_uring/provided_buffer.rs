use std::os::fd::RawFd;

use io_uring::{buf_ring::BufRing, cqueue::Entry, opcode, squeue, types, CompletionQueue, IoUring};
use libc::msghdr;
use log::{debug, warn};

use crate::{util::statistic::{Parameter, UringParameter}, Statistic};

use super::IoUringOperatingModes;

pub struct IoUringProvidedBuffer {
    ring: IoUring,
    buf_ring: BufRing,
    parameter: UringParameter,
    msghdr: msghdr,
    statistic: Statistic
}

impl IoUringProvidedBuffer {
    pub fn submit(&mut self, amount_recvmsg: u32, socket_fd: i32) -> Result<u32, &'static str> {
        let mut submission_count = 0;
        let mut sq = self.ring.submission();
        debug!("BEGIN io_uring_submit: Current sq len: {}. Dropped messages: {}", sq.len(), sq.dropped());

        for _ in 0..amount_recvmsg {
            let sqe = opcode::RecvMsg::new(types::Fd(socket_fd), &mut self.msghdr)
                .buf_group(crate::URING_BUFFER_GROUP) 
                .build()
                .flags(squeue::Flags::BUFFER_SELECT);

            match unsafe { sq.push(&sqe) } {
                Ok(_) => submission_count += 1,
                Err(err) => {
                    // When using submission queue polling, it can happen that the reported queue length is not the same as the actual queue length.
                    warn!("Error pushing io_uring sqe: {}. Stopping submit() after submitting {} entries", err, submission_count);
                    break;
                }
            };
        }

        debug!("END io_uring_submit: Submitted {} io_uring sqe. Current sq len: {}. Dropped messages: {}", submission_count, sq.len(), sq.dropped());
        Ok(submission_count)
    }

    // TODO: Could be generic function: Only self type differs 
    pub fn fill_sq_and_submit(&mut self, amount_inflight: u32, socket_fd: i32) -> Result<u32, &'static str> {
        let mut amount_new_requests = 0;

        let min_complete = match super::calc_sq_fill_mode(amount_inflight, self.parameter, &mut self.ring) {
            (0,0) => return Ok(0),
            (to_submit, min_complete) => {
                amount_new_requests += self.submit(to_submit, socket_fd)?;
                min_complete
            }
        };

        // Utilization of the submission queue
        self.statistic.uring_sq_utilization[self.ring.submission().len()] += 1;
        Self::io_uring_enter(&mut self.ring.submitter(), crate::URING_ENTER_TIMEOUT, min_complete)?;

        // Utilization of the completion queue
        self.statistic.uring_cq_utilization[self.ring.completion().len()] += 1;
        debug!("Added {} new requests to submission queue. Current inflight: {}", amount_new_requests, amount_inflight + amount_new_requests);

        Ok(amount_new_requests)
    }

    pub fn get_bufs_and_cq(&mut self) -> (&mut BufRing, CompletionQueue<'_, Entry>) {
        (&mut self.buf_ring, self.ring.completion())
    }
}

impl IoUringOperatingModes for IoUringProvidedBuffer {
    type Mode = IoUringProvidedBuffer;

    fn new(parameter: Parameter, io_uring_fd: Option<RawFd>) -> Result<Self, &'static str> {
        let ring = super::create_ring(parameter.uring_parameter, io_uring_fd)?;
        let buf_ring = super::create_buf_ring(&mut ring.submitter(), parameter.uring_parameter.buffer_size as u16, parameter.mss);

        // Generic msghdr: msg_controllen and msg_namelen relevant, when using provided buffers
        // https://github.com/SUPERCILEX/clipboard-history/blob/418b2612f8e62693e42057029df78f6fbf49de3e/server/src/reactor.rs#L206
        // https://github.com/axboe/liburing/blob/cc61897b928e90c4391e0d6390933dbc9088d98f/examples/io_uring-udp.c#L113
        let msghdr = {
            let mut hdr = unsafe { std::mem::zeroed::<libc::msghdr>() };
            hdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
            hdr
        };

        Ok(IoUringProvidedBuffer {
            ring,
            buf_ring,
            parameter: parameter.uring_parameter,
            msghdr,
            statistic: Statistic::new(parameter)
        })
    }

    fn get_statistic(&self) -> Statistic {
        self.statistic.clone()
    }
}