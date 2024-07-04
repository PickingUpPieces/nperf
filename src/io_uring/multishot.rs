use std::os::fd::RawFd;
use io_uring::{buf_ring::BufRing, cqueue::Entry, opcode, types, CompletionQueue, IoUring};
use libc::msghdr;
use log::{debug, error};

use crate::{util::statistic::{Parameter, UringParameter}, Statistic};

use super::IoUringOperatingModes;

pub struct IoUringMultishot {
    ring: IoUring,
    buf_ring: BufRing,
    _parameter: UringParameter,
    msghdr: msghdr,
    statistic: Statistic
}

impl IoUringMultishot {
    fn submit(&mut self, socket_fd: i32) -> Result<u32, &'static str> {
        // Use the socket file descripter to receive messages
        debug!("Arming multishot request");
        let sqe = opcode::RecvMsgMulti::new(types::Fd(socket_fd), &self.msghdr, crate::URING_BUFFER_GROUP).build();

        match unsafe { self.ring.submission().push(&sqe) } {
            Ok(_) => {
                Ok(1)
            },
            Err(err) => {
                error!("Error pushing io_uring sqe: {}", err);
                Err("IO_URING ERROR")
            }
        }
    }

    pub fn fill_sq_and_submit(&mut self, armed: bool, socket_fd: i32) -> Result<u32, &'static str> {
        let mut amount_new_requests = 0;
        if !armed {
            amount_new_requests = self.submit(socket_fd)?;
            // Utilization of the submission queue
            if let Some(ref mut array) = self.statistic.uring_sq_utilization {
                array[self.ring.submission().len()] += 1;
            }
        };

        // Weird bug, if min_complete bigger than 1, submit_and_wait does NOT return the timeout error, but actually takes as long as the timeout error and returns then 1.
        // Due to this bug, we have less batching effects. 
        // Normally we want here the parameter: self.parameter.uring_parameter.burst_size as usize
        Self::io_uring_enter(&mut self.ring.submitter(), crate::URING_ENTER_TIMEOUT, 1)?;

        // Utilization of the completion queue
        if let Some(ref mut array) = self.statistic.uring_cq_utilization {
            array[self.ring.completion().len()] += 1;
        }

        Ok(amount_new_requests)
    }


    pub fn get_bufs_and_cq(&mut self) -> (&mut BufRing, CompletionQueue<'_, Entry>) {
        (&mut self.buf_ring, self.ring.completion())
    }

    pub fn get_msghdr(&self) -> msghdr {
        self.msghdr
    }
}

impl IoUringOperatingModes for IoUringMultishot {
    type Mode = IoUringMultishot;

    fn new(parameter: Parameter, io_uring_fd: Option<RawFd>) -> Result<Self, &'static str> {
        let ring = super::create_ring(parameter.uring_parameter, io_uring_fd)?;
        let buf_ring = super::create_buf_ring(&mut ring.submitter(), parameter.uring_parameter.buffer_size as u16, parameter.mss);

        // Generic msghdr: msg_controllen and msg_namelen relevant, when using provided buffers
        let msghdr = {
            let mut hdr = unsafe { std::mem::zeroed::<libc::msghdr>() };
            hdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
            hdr
        };

        Ok(IoUringMultishot {
            ring,
            buf_ring,
            _parameter: parameter.uring_parameter,
            msghdr,
            statistic: Statistic::new(parameter)
        })
    }

    fn get_statistic(&self) -> Statistic {
        self.statistic.clone()
    }

    fn reset_statistic(&mut self, parameter: Parameter) {
        self.statistic = Statistic::new(parameter);
    }
}