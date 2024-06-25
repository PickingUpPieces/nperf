pub mod normal;
pub mod provided_buffer;
pub mod multishot;
pub mod send;

use std::os::fd::RawFd;
use io_uring::{buf_ring::BufRing, cqueue, opcode, types::{SubmitArgs, Timespec}, IoUring, Probe, Submitter};
use log::{debug, error, info, warn};
use serde::Serialize;
use crate::{util::statistic::{Parameter, UringParameter}, Statistic};

const URING_SQ_POLL_TIMEOUT: u32 = 2_000;
pub const IORING_CQE_F_NOTIF: u32 = 8;

#[derive(clap::ValueEnum, Debug, PartialEq, Serialize, Clone, Copy, Default)]
pub enum UringSqFillingMode {
    #[default]
    Topup,
    TopupNoWait,
    Syscall 
}

#[derive(clap::ValueEnum, Debug, PartialEq, Serialize, Clone, Copy, Default)]
pub enum UringTaskWork {
    #[default]
    Default,
    Coop,
    Defer,
    CoopDefer
}

#[derive(clap::ValueEnum, Debug, PartialEq, Serialize, Clone, Copy, Default)]
pub enum UringMode {
    #[default]
    Normal,
    Zerocopy,
    ProvidedBuffer,
    Multishot
}

pub trait IoUringOperatingModes {
    type Mode;

    fn new(parameter: Parameter, io_uring_fd: Option<RawFd>) -> Result<Self::Mode, &'static str>;

    fn get_statistic(&self) -> Statistic;

    fn io_uring_enter(submitter: &mut Submitter, timeout: u32, min_complete: usize) -> Result<(), &'static str> {
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
}

fn create_ring(parameters: UringParameter, io_uring_fd: Option<RawFd>) -> Result<IoUring, &'static str> {
        info!("Setting up io_uring with burst size: {}, and sq ring size: {}", parameters.burst_size, parameters.ring_size);

        let mut ring_builder = IoUring::<io_uring::squeue::Entry>::builder();

        info!("Setting up io_uring with SINGLE_ISSUER");
        ring_builder.setup_single_issuer();

        if parameters.task_work == UringTaskWork::Coop {
            info!("Setting up io_uring with cooperative task work (IORING_SETUP_COOP_TASKRUN)");
            ring_builder.setup_coop_taskrun();
        } else if parameters.task_work == UringTaskWork::Defer {
            info!("Setting up io_uring with deferred task work (IORING_SETUP_DEFER_TASKRUN)");
            ring_builder.setup_defer_taskrun();
        } else if parameters.task_work == UringTaskWork::CoopDefer {
            info!("Setting up io_uring with cooperative and deferred task work (IORING_SETUP_COOP_TASKRUN | IORING_SETUP_DEFER_TASKRUN)");
            ring_builder.setup_coop_taskrun().setup_defer_taskrun();
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
                    info!("Starting uring with SQ_POLL thread. Pinned to CPU: {}. Poll timeout: {}ms", crate::URING_SQPOLL_CPU, URING_SQ_POLL_TIMEOUT);
                    ring_builder
                    .setup_sqpoll(URING_SQ_POLL_TIMEOUT)
                    .setup_sqpoll_cpu(crate::URING_SQPOLL_CPU); // CPU to run the SQ poll thread on core 0 by default
                }
            }
        };

        let mut ring = ring_builder.build(parameters.ring_size)
            .map_err(|_| "Failed to create io_uring")?;

        let sq_cap = ring.submission().capacity();
        debug!("Created io_uring instance successfully with CQ size: {} and SQ size: {}", ring.completion().capacity(), sq_cap);
        check_io_uring_features_available(&ring, parameters)?;

        Ok(ring)
}

fn create_buf_ring(submitter: &mut Submitter, buffer_size: u16, mss: u32) -> BufRing {
    let ring_buf = submitter
    // In multishot mode, more parts of the msghdr struct are written into the buffer, so we need to allocate more space ( + crate::URING_ADDITIONAL_BUFFER_LENGTH )
    .register_buf_ring(buffer_size, crate::URING_BUFFER_GROUP, mss + crate::URING_ADDITIONAL_BUFFER_LENGTH as u32)
    .expect("Creation of BufRing failed.");

    debug!("Registered buffer ring at io_uring instance with capacity: {} and single buffer size: {}", buffer_size, mss + crate::URING_ADDITIONAL_BUFFER_LENGTH as u32);
    ring_buf
}


// Check flags, if multishot request is still armed
pub fn check_multishot_status(flags: u32) -> bool {
    if !cqueue::more(flags) {
        debug!("Missing flag IORING_CQE_F_MORE indicating, that multishot has been disarmed or in case of using send zero-copy, that the buffer can be reused.");
        false
    } else {
        true
    }
}

fn calc_sq_fill_mode(amount_inflight: u32, parameter: UringParameter, ring: &mut IoUring) -> (usize, usize) {
    let uring_burst_size = parameter.burst_size;
    let uring_buffer_size = parameter.buffer_size;
    let min_complete;
    let mut to_submit: u32 = 0;

    // Check if there are not enough free buffers -> Either wait for completion events or reap them
    if amount_inflight > uring_buffer_size - uring_burst_size {
        if ring.completion().is_empty() {
            // No buffers left and cq is empty -> We need to do some work/wait for CQEs
            min_complete = if uring_burst_size == 0 {
                parameter.ring_size / crate::URING_BURST_SIZE_DIVIDEND // Default burst size
            }  else {
                uring_burst_size
            } as usize;
        } else {
            // If no buffers left, but CQE events in CQ, we don't want to call io_uring_enter -> exit
            return (0,0);
        }
    } else {
        // There are enough buffers left to fill up the submission queue
        match parameter.sq_filling_mode {
            UringSqFillingMode::Syscall => {
                // Check if the submission queue is max filled with the burst size
                if amount_inflight < uring_burst_size {
                    to_submit = uring_burst_size;
                } else {
                    // If there are currently more entries inflight than the burst size, we don't want to submit more entries
                    to_submit = 0;
                }
            },
            UringSqFillingMode::Topup | UringSqFillingMode::TopupNoWait => {
                // Fill up the submission queue to the maximum
                let sq_entries_left = {
                    let sq = ring.submission();
                    sq.capacity() - sq.len()
                } as u32;
                let buffers_left = uring_buffer_size - amount_inflight;
                // Check if enough buffers are left to fill up the submission queue, otherwise only fill up the remaining buffers
                if buffers_left < sq_entries_left {
                    to_submit = buffers_left;
                } else {
                    to_submit = sq_entries_left;
                }
            }
        };

        // SQ_POLL: Only reason to call io_uring_enter is to wake up SQ_POLL thread.
        //          Due to the library we're using, the library function will only trigger the syscall io_uring_enter, if the sq_poll thread is asleep.
        //          If min_complete > 0, io_uring_enter syscall is triggered, so for SQ_POLL we don't want this normally.
        //          If other task_work is implemented, we need to force this probably.
        min_complete = if parameter.sqpoll || parameter.sq_filling_mode == UringSqFillingMode::TopupNoWait { 0 } else { uring_burst_size } as usize;
    }
    (to_submit as usize, min_complete)
}


// This function is used to parse the amount of bytes received from the socket
// It shall only be used for io_uring, since the error codes are different (negated).
// Errors are negated, since a positive amount of bytes received is a success.
// io_uring doesn't use the errno variable, but returns the error code directly.
pub fn parse_received_bytes(amount_received_bytes: i32) -> Result<u32, &'static str> {
    match amount_received_bytes {
        -105 => { // result is -105, libc::ENOBUFS, no buffer space available (https://github.com/tokio-rs/io-uring/blob/b29e81583ed9a2c35feb1ba6f550ac1abf398f48/src/squeue.rs#L87) -> Only needed for provided buffers
            warn!("ENOBUFS: No buffer space available!");
            Ok(0)
        },
        -11 => {
            // If no messages are available at the socket, the receive calls wait for a message to arrive, unless the socket is nonblocking (see fcntl(2)), in which case the value -11 is returned and the external variable errno is set to EAGAIN or EWOULDBLOCK.
            // From: https://linux.die.net/man/2/recvmsg
            // libc::EAGAIN == 11
            debug!("EAGAIN: No messages available at the socket!"); // This should not happen in io_uring with FAST_POLL
            Err("EAGAIN")
        },
        -90 => { // libc::EMSGSIZE -> Message too long
            warn!("EMSGSIZE: The message is too long to fit into the supplied buffer and was truncated.");
            Ok(0)
        },
        _ if amount_received_bytes < 0 => {
            error!("Error receiving message! Negated error code: {}", amount_received_bytes);
            Err("Failed to receive data!")
        },
        _ => Ok(1) // Positive amount of bytes received
    }
}

fn check_io_uring_features_available(ring: &IoUring, parameter: UringParameter) -> Result<(), &'static str> {
    let mut probe = Probe::new();
    if ring.submitter().register_probe(&mut probe).is_err() {
        warn!("Unable to check for availability of io-uring features, since probe is not supported!");
        return Ok(());
    }

    if parameter.uring_mode == UringMode::Multishot {
        if !probe.is_supported(opcode::RecvMsgMulti::CODE) {
            return Err("IORING_OP_RECVMSG_MULTI is not supported in the kernel!");
        }
        if !probe.is_supported(opcode::ProvideBuffers::CODE) {
            return Err("IORING_OP_PROVIDE_BUFFERS is not supported in the kernel!");
        }
    } else if parameter.uring_mode == UringMode::ProvidedBuffer && !probe.is_supported(opcode::ProvideBuffers::CODE) {
        return Err("IORING_OP_PROVIDE_BUFFERS is not supported in the kernel!");
    } else if parameter.uring_mode == UringMode::Zerocopy && !probe.is_supported(opcode::SendMsgZc::CODE) {
        return Err("IORING_OP_SENDMSG_ZC is not supported in the kernel!");
    }

    if !ring.params().is_feature_fast_poll() {
        warn!("IORING_FEAT_FAST_POLL is NOT available in the kernel!");
    } else {
        info!("IORING_FEAT_FAST_POLL is available and used!");
    }
    Ok(())
}