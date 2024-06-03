use std::os::fd::RawFd;

use io_uring::{cqueue::Entry, opcode, types, CompletionQueue, IoUring};
use log::{debug, trace, warn};

use crate::{util::{packet_buffer::PacketBuffer, statistic::{Parameter, UringParameter}}, Statistic};

use super::IoUringOperatingModes;
pub struct IoUringSend {
    ring: IoUring,
    parameter: UringParameter,
    statistic: Statistic
}

impl IoUringSend {
    fn submit(&mut self, amount_requests: usize, packet_buffer: &mut PacketBuffer, next_packet_id: u64, socket_fd: i32) -> Result<usize, &'static str> {
        let mut submission_count = 0;
        let mut sq = self.ring.submission();
        debug!("BEGIN io_uring_submit: Current sq len: {}. Dropped messages: {}", sq.len(), sq.dropped());

        // Add all packet_ids in one go -> Probably more efficient, but not benched
        let assigned_packet_ids = packet_buffer.add_packet_ids(next_packet_id, Some(amount_requests))?;
        // TODO: Needs to be relaxed for GSRO, since we will just round up to the next multiple of packets_per_buffer
        assert!(assigned_packet_ids == amount_requests as u64); 
        let packets_per_buffer = packet_buffer.packets_amount_per_msghdr();

        for i in 0..amount_requests {
            let packet_id = next_packet_id + ( i * packets_per_buffer ) as u64;
            trace!("Message number {}/{}: Used buffer index {}", i, amount_requests, i);

            let sqe = opcode::SendMsg::new(types::Fd(socket_fd), packet_buffer.get_msghdr_from_index(i)?)
            .build()
            .user_data(packet_id);

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

    pub fn fill_sq_and_submit(&mut self, amount_inflight: usize, packet_buffer: &mut PacketBuffer, next_packet_id: u64, socket_fd: i32) -> Result<usize, &'static str> {
        let mut amount_new_requests = 0;

        let min_complete = match super::calc_sq_fill_mode(amount_inflight as u32, self.parameter, &mut self.ring) {
            (0,0) => return Ok(0),
            (to_submit, min_complete) => {
                amount_new_requests += self.submit(to_submit, packet_buffer, next_packet_id, socket_fd)?;
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
}

impl IoUringOperatingModes for IoUringSend {
    type Mode = IoUringSend;

    fn new(parameter: Parameter, io_uring_fd: Option<RawFd>) -> Result<IoUringSend, &'static str> {
        let ring = super::create_ring(parameter.uring_parameter, io_uring_fd)?;

        Ok(IoUringSend {
            ring,
            parameter: parameter.uring_parameter,
            statistic: Statistic::new(parameter)
        })
    }

    fn get_statistic(&self) -> Statistic {
        self.statistic.clone()
    }
}