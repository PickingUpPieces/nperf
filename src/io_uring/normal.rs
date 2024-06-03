use io_uring::{cqueue::Entry, opcode, types, CompletionQueue, IoUring};
use log::{debug, trace, warn};
use std::os::{fd::RawFd, unix::io::AsRawFd};

use crate::{util::{packet_buffer::PacketBuffer, statistic::{Parameter, UringParameter}}, Statistic};

use super::IoUringOperatingModes;
pub struct IoUringNormal {
    ring: IoUring,
    parameter: UringParameter,
    statistic: Statistic
}

impl IoUringNormal {
    pub fn submit(&mut self, amount_recvmsg: u32, packet_buffer: &mut PacketBuffer, socket_fd: i32) -> Result<u32, &'static str> {
        let mut submission_count = 0;
        let mut sq = self.ring.submission();
        debug!("BEGIN io_uring_submit: Current sq len: {}. Dropped messages: {}", sq.len(), sq.dropped());

        for i in 0..amount_recvmsg {
            let packet_buffer_index = packet_buffer.get_buffer_index()?;
            trace!("Message number {}/{}: Used buffer index {}", i, amount_recvmsg, packet_buffer_index);

            let sqe = opcode::RecvMsg::new(types::Fd(socket_fd), packet_buffer.get_msghdr_from_index(packet_buffer_index)?)
            .build()
            .user_data(packet_buffer_index as u64);

            match unsafe { sq.push(&sqe) } {
                Ok(_) => submission_count += 1,
                Err(err) => {
                    // When using submission queue polling, it can happen that the reported queue length is not the same as the actual queue length.
                    warn!("Error pushing io_uring sqe: {}. Stopping submit() after submitting {} entries", err, submission_count);
                    packet_buffer.return_buffer_index(vec![packet_buffer_index]);
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
        Self::io_uring_enter(&mut self.ring.submitter(), crate::URING_ENTER_TIMEOUT, min_complete)?;

        // Utilization of the completion queue
        self.statistic.uring_cq_utilization[self.ring.completion().len()] += 1;
        debug!("Added {} new requests to submission queue. Current inflight: {}", amount_new_requests, amount_inflight + amount_new_requests);

        Ok(amount_new_requests)
    }

    pub fn get_cq(&mut self) -> CompletionQueue<'_, Entry> {
        self.ring.completion()
    }

    pub fn get_raw_fd(&self) -> i32 {
        self.ring.as_raw_fd()
    }
}

impl IoUringOperatingModes for IoUringNormal {
    type Mode = IoUringNormal;

    fn new(parameter: Parameter, io_uring_fd: Option<RawFd>) -> Result<IoUringNormal, &'static str> {
        let ring = super::create_ring(parameter.uring_parameter, io_uring_fd)?;

        Ok(IoUringNormal {
            ring,
            parameter: parameter.uring_parameter,
            statistic: Statistic::new(parameter)
        })
    }

    fn get_statistic(&self) -> Statistic {
        self.statistic.clone()
    }
}