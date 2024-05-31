use std::os::fd::RawFd;

use io_uring::{buf_ring::BufRing, IoUring};
use log::{debug, info, warn};

use crate::util::statistic::UringParameter;

const URING_TASK_WORK: bool = false;
const URING_SQ_POLL_TIMEOUT: u32 = 2_000;

pub fn io_uring_setup(mss: u32, parameters: UringParameter, io_uring_fd: Option<RawFd>) -> Result<(IoUring, Option<BufRing>), &'static str> {
        info!("Setup io_uring with burst size: {}, buffer length: {}, single buffer size: {} and sq ring size: {}", parameters.burst_size, parameters.buffer_size, mss, parameters.ring_size);

        let mut ring_builder = IoUring::<io_uring::squeue::Entry>::builder();

        if URING_TASK_WORK {
            ring_builder
            .setup_coop_taskrun()
            .setup_single_issuer()
            .setup_defer_taskrun();
        }

        if parameters.sqpoll {
            match io_uring_fd {
                Some(fd) => {
                    info!("Using existing SQ_POLL thread from io_uring instance: {}", fd);
                    ring_builder
                    .setup_sqpoll(URING_SQ_POLL_TIMEOUT)
                    .setup_attach_wq(fd);
                },
                None => {
                    const SQPOLL_CPU: u32 = 0;
                    info!("Starting uring with SQ_POLL thread. Pinned to CPU: {}. Poll timeout: {}ms", SQPOLL_CPU, URING_SQ_POLL_TIMEOUT);
                    ring_builder
                    .setup_sqpoll(URING_SQ_POLL_TIMEOUT)
                    .setup_sqpoll_cpu(SQPOLL_CPU); // CPU to run the SQ poll thread on core 0 by default
                }
            }
        };

        let mut ring = ring_builder.build(parameters.ring_size).expect("Failed to create io_uring");
        let sq_cap = ring.submission().capacity();
        debug!("Created io_uring instance successfully with CQ size: {} and SQ size: {}", ring.completion().capacity(), sq_cap);

        if !ring.params().is_feature_fast_poll() {
            warn!("IORING_FEAT_FAST_POLL is NOT available in the kernel!");
        } else {
            info!("IORING_FEAT_FAST_POLL is available and used!");
        }

        // Register provided buffers with io_uring
        let buf_ring = if parameters.provided_buffer {
            let buf_ring = ring.submitter()
            // In multishot mode, more parts of the msghdr struct are written into the buffer, so we need to allocate more space ( + crate::URING_ADDITIONAL_BUFFER_LENGTH )
            .register_buf_ring(u16::try_from(parameters.buffer_size).unwrap(), crate::URING_BUFFER_GROUP, mss + crate::URING_ADDITIONAL_BUFFER_LENGTH as u32)
            .expect("Creation of BufRing failed.");
            debug!("Registered buffer ring at io_uring instance with capacity: {} and single buffer size: {}", parameters.buffer_size, mss + crate::URING_ADDITIONAL_BUFFER_LENGTH as u32);
            Some(buf_ring)
        } else {
            None
        };

        Ok((ring, buf_ring))
}