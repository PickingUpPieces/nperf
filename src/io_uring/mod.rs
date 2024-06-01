pub mod normal;
//pub mod provided_buffer;
//pub mod multishot;

use std::os::fd::RawFd;
use io_uring::{buf_ring::BufRing, cqueue, types::{SubmitArgs, Timespec}, IoUring, Submitter};
use log::{debug, error, info, warn};
use serde::Serialize;
use crate::util::statistic::UringParameter;

const URING_SQ_POLL_TIMEOUT: u32 = 2_000;

#[derive(clap::ValueEnum, Debug, PartialEq, Serialize, Clone, Copy, Default)]
pub enum UringSqFillingMode {
    #[default]
    Topup,
    Syscall 
}

#[derive(clap::ValueEnum, Debug, PartialEq, Serialize, Clone, Copy, Default)]
pub enum UringTaskWork {
    Default,
    Coop,
    #[default]
    Defer
}


pub fn create_ring(parameters: UringParameter, io_uring_fd: Option<RawFd>) -> Result<IoUring, &'static str> {
        info!("Setup io_uring with burst size: {}, and sq ring size: {}", parameters.burst_size, parameters.ring_size);

        let mut ring_builder = IoUring::<io_uring::squeue::Entry>::builder();

        if parameters.task_work == UringTaskWork::Coop {
            info!("Setting up io_uring with cooperative task work (IORING_SETUP_COOP_TASKRUN)");
            ring_builder.setup_coop_taskrun();
        } else if parameters.task_work == UringTaskWork::Defer {
            info!("Setting up io_uring with deferred task work (IORING_SETUP_DEFER_TASKRUN)");
            ring_builder.setup_single_issuer()
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

        let mut ring = ring_builder.build(parameters.ring_size)
            .map_err(|_| "Failed to create io_uring")?;

        let sq_cap = ring.submission().capacity();
        debug!("Created io_uring instance successfully with CQ size: {} and SQ size: {}", ring.completion().capacity(), sq_cap);

        if !ring.params().is_feature_fast_poll() {
            warn!("IORING_FEAT_FAST_POLL is NOT available in the kernel!");
        } else {
            info!("IORING_FEAT_FAST_POLL is available and used!");
        }

        Ok(ring)
}

pub fn create_buf_ring(submitter: &mut Submitter, buffer_size: u16, mss: u32) -> BufRing {
    let ring_buf = submitter
    // In multishot mode, more parts of the msghdr struct are written into the buffer, so we need to allocate more space ( + crate::URING_ADDITIONAL_BUFFER_LENGTH )
    .register_buf_ring(buffer_size, crate::URING_BUFFER_GROUP, mss + crate::URING_ADDITIONAL_BUFFER_LENGTH as u32)
    .expect("Creation of BufRing failed.");

    debug!("Registered buffer ring at io_uring instance with capacity: {} and single buffer size: {}", buffer_size, mss + crate::URING_ADDITIONAL_BUFFER_LENGTH as u32);

    ring_buf
}

pub fn io_uring_enter(submitter: &mut Submitter, timeout: u32, min_complete: usize) -> Result<(), &'static str> {
    // Simulates https://man7.org/linux/man-pages/man3/io_uring_submit_and_wait_timeout.3.html
    // Submit to kernel and wait for completion event or timeout. In case the thread doesn't receive any messages.
    let mut args = SubmitArgs::new();
    let ts = Timespec::new().nsec(timeout);
    args = args.timespec(&ts);

    match if timeout == 0 { submitter.submit_and_wait(min_complete) } else { submitter.submit_with_args(min_complete, &args) } {
        Ok(submitted) => { debug!("Amount of submitted events received from submit: {}", submitted) },
        // If this overflow condition is entered, attempting to submit more IO with fail with the -EBUSY error value, if it can’t flush the overflown events to the CQ ring. 
        // If this happens, the application must reap events from the CQ ring and attempt the submit again.
        // Should ONLY appear when using flag IORING_FEAT_NODROP
        Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => { warn!("submit_with_args: EBUSY error") },
        Err(ref err) if err.raw_os_error() == Some(62) => { debug!("submit_with_args: Timeout error") },
        Err(err) => {
            error!("Error submitting io_uring sqe: {}", err);
            return Err("IO_URING_ERROR")
        }
    }

    Ok(())
}

// Check flags, if multishot request is still armed
pub fn check_multishot_status(flags: u32) -> bool {
    if !cqueue::more(flags) {
        debug!("Missing flag IORING_CQE_F_MORE indicating, that multishot has been disarmed");
        false
    } else {
        true
    }
}

