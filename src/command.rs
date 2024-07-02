use std::path;

use clap::Parser;
use log::{error, info, warn};

use crate::{io_uring::{UringMode, UringSqFillingMode, UringTaskWork}, util::{self, statistic::{MultiplexPort, OutputFormat, Parameter, SimulateConnection, UringParameter}, ExchangeFunction, IOModel, NPerfMode}};
use crate::net::{self, socket_options::SocketOptions};

#[derive(Parser,Default,Debug)]
#[clap(version, about="A network performance measurement tool")]
#[allow(non_camel_case_types)]
pub struct nPerf {
    /// Mode of operation: sender or receiver
    #[arg(default_value_t, value_enum)]
    mode: NPerfMode,

    /// IP address to measure against/listen on
    #[arg(short = 'a',long, default_value_t = String::from("0.0.0.0"))]
    ip: String,

    /// Port number to measure against, receiver listen on.
    #[arg(short, long, default_value_t = crate::DEFAULT_RECEIVER_PORT)]
    pub port: u16,

    /// Port number senders send from.
    #[arg(short, long, default_value_t = crate::DEFAULT_SENDER_PORT)]
    pub sender_port: u16,

    /// Start multiple sender/receiver threads in parallel. The port number will be incremented automatically.
    #[arg(long, default_value_t = 1)]
    parallel: u16,

    /// Don't stop the node after the first measurement
    #[arg(short, long, default_value_t = false)]
    pub run_infinite: bool,

    /// Interval printouts of the statistic in seconds (0 to disable).
    #[arg(short, long, default_value_t = crate::DEFAULT_INTERVAL)]
    interval: f64,

    /// Set length of single datagram (Without IP and UDP headers)
    #[arg(short = 'l', long, default_value_t = crate::DEFAULT_UDP_DATAGRAM_SIZE)]
    datagram_size: u32,

    /// Amount of seconds to run the test for
    #[arg(short = 't', long, default_value_t = crate::DEFAULT_DURATION)]
    time: u64,

    /// Pin each thread to an individual core. The receiver threads start from the last core, the sender threads from the second core. This way each receiver/sender pair should operate on the same NUMA core.
    #[arg(long, default_value_t = false)]
    with_core_affinity: bool,

    /// Pin sender/receiver threads to different NUMA nodes
    #[arg(long, default_value_t = false)]
    with_numa_affinity: bool,

    /// Enable GSO/GRO on socket
    #[arg(long, default_value_t = false)]
    with_gsro: bool,

    /// Enable socket pacing with SO_MAX_PACING_RATE with the given rate in Mbit/s (0 for disabled)
    #[arg(long, default_value_t = crate::DEFAULT_SOCKET_PACING)]
    with_socket_pacing_rate: u32,

    /// Set GSO buffer size which overwrites the MSS by default if GSO/GRO is enabled
    #[arg(long, default_value_t = crate::DEFAULT_GSO_BUFFER_SIZE)]
    with_gso_buffer: u32,

    /// Set transmit buffer size. Gets overwritten by GSO/GRO buffer size if GSO/GRO is enabled.
    #[arg(long, default_value_t = crate::DEFAULT_MSS)]
    with_mss: u32,

    /// Disable fragmentation on sending socket
    #[arg(long, default_value_t = false)]
    with_ip_frag: bool,

    /// Disable non-blocking socket
    #[arg(long, default_value_t = false)]
    without_non_blocking: bool,

    /// Enable setting udp socket buffer size
    #[arg(long, default_value_t = false)]
    with_socket_buffer: bool,

    /// Exchange function to use: normal (send/recv), sendmsg/recvmsg, sendmmsg/recvmmsg
    #[arg(long, default_value_t, value_enum)]
    exchange_function: ExchangeFunction,
    
    /// Amount of message packs of gso_buffers to send when using sendmmsg
    #[arg(long, default_value_t = crate::DEFAULT_AMOUNT_MSG_WHEN_SENDMMSG)]
    with_mmsg_amount: usize,

    /// Select the IO model to use: busy-waiting, select, poll
    #[arg(long, default_value_t, value_enum)]
    io_model: IOModel,

    /// Define the type the output 
    #[arg(long, default_value_t, value_enum)]
    output_format: OutputFormat,

    /// Define the path in which the results file should be saved. Make sure the path exists and the application has the rights to write in it.
    #[arg(long, default_value = crate::DEFAULT_FILE_NAME)]
    output_file_path: path::PathBuf,

    /// Test label which appears in the output file, if multiple tests are run in parallel
    #[arg(long, default_value_t = String::from("nperf-test"))]
    label_test: String,

    /// Run label which appears in the output file, to differentiate between multiple different runs which are executed within a single test
    #[arg(long, default_value_t = String::from("run-nperf"))]
    label_run: String,

    /// Use different port number for each sender thread, share a single port or shard a single port with reuseport
    #[arg(long, default_value_t, value_enum)]
    multiplex_port: MultiplexPort,

    /// Same as for multiplex_port, but for the receiver
    #[arg(long, default_value_t, value_enum)]
    multiplex_port_receiver: MultiplexPort,

    /// CURRENTLY IGNORED. Simulate a single QUIC connection or one QUIC connection per thread.
    #[arg(long, default_value_t, value_enum)]
    simulate_connection: SimulateConnection,

    /// io_uring: Which mode to use
    #[arg(long, default_value_t, value_enum)]
    uring_mode: UringMode,

    /// io_uring: Use a SQ_POLL thread per executing thread, pinned to CPU 0
    #[arg(long, default_value_t = false)]
    uring_sqpoll: bool,

    /// io_uring: Share the SQ_POLL thread between all executing threads
    #[arg(long, default_value_t = false)]
    uring_sqpoll_shared: bool,

    /// io_uring: Amount of recvmsg/sendmsg requests are submitted/completed in one go
    #[arg(long, default_value_t = crate::DEFAULT_URING_RING_SIZE / crate::URING_BURST_SIZE_DIVIDEND)]
    uring_burst_size: u32,

    /// io_uring: Size of the ring buffer
    #[arg(long, default_value_t = crate::DEFAULT_URING_RING_SIZE)]
    uring_ring_size: u32,

    /// io_uring: How the SQ is filled
    #[arg(long, default_value_t, value_enum)]
    uring_sq_mode: UringSqFillingMode,

    /// io_uring: Set the operation mode of task_work
    #[arg(long, default_value_t, value_enum)]
    uring_task_work: UringTaskWork,

    /// Show help in markdown format
    #[arg(long, hide = true)]
    markdown_help: bool,
}

impl nPerf {
    pub fn new() -> Self {
        let _ = env_logger::try_init();
        nPerf::parse()
    }

    pub fn set_args(self, args: Vec<&str>) -> Self {
        let mut args = args;
        args.insert(0, "nPerf");
        let args: Vec<String> = args.iter().map(|x| x.to_string()).collect();
        nPerf::parse_from(args)
    }

    pub fn parse_parameter(&self) -> Option<util::statistic::Parameter> {
        if self.markdown_help {
            clap_markdown::print_help_markdown::<nPerf>();
            return None;
        }
    
        let ipv4 = match net::parse_ipv4(&self.ip) {
            Ok(x) => x,
            Err(_) => { error!("Invalid IPv4 address!"); return None; },
        };
    
        let packet_buffer_size = match self.exchange_function {
            ExchangeFunction::Mmsg => self.with_mmsg_amount,
            _ => 1,
        };

        let mss = if self.with_gsro {
            info!("GSO/GRO enabled with buffer size {}", self.with_gso_buffer);
            self.with_gso_buffer
        } else {
            self.with_mss
        };

        let simulate_connection = match self.multiplex_port_receiver {
            MultiplexPort::Sharing => SimulateConnection::Single,
            _ => SimulateConnection::Multiple
        };

        info!("Simulate connection: {:?}", simulate_connection);
        info!("Exchange function used: {:?}", self.exchange_function);
        info!("MSS used: {}", mss);
        info!("IO model used: {:?}", self.io_model);
        info!("UDP datagram size used: {}", self.datagram_size);
        info!("Socket pacing rate: {} Mbit/s", self.with_socket_pacing_rate);

        let socket_options = self.parse_socket_options(self.mode);

        let uring_parameters = UringParameter {
            uring_mode: self.uring_mode,
            ring_size: self.uring_ring_size,
            burst_size: if self.uring_burst_size == crate::DEFAULT_URING_RING_SIZE / crate::URING_BURST_SIZE_DIVIDEND { (self.uring_ring_size as f32 / crate::URING_BURST_SIZE_DIVIDEND as f32).ceil() as u32 } else { self.uring_burst_size } ,
            buffer_size: self.uring_ring_size * crate::URING_BUFFER_SIZE_MULTIPLICATOR,
            sqpoll: self.uring_sqpoll,
            sqpoll_shared: self.uring_sqpoll_shared,
            sq_filling_mode: self.uring_sq_mode,
            task_work: self.uring_task_work,
        };

        let parameter = util::statistic::Parameter::new(
            self.label_test.clone(),
            self.label_run.clone(),
            self.mode, 
            ipv4, 
            self.parallel,
            self.interval,
            self.output_format, 
            self.output_file_path.clone(),
            self.io_model, 
            self.time, 
            mss, 
            self.datagram_size, 
            packet_buffer_size, 
            socket_options, 
            self.exchange_function,
            self.multiplex_port,
            self.multiplex_port_receiver,
            simulate_connection,
            self.with_core_affinity,
            self.with_numa_affinity,
            uring_parameters
        );

        self.parameter_check(parameter) 
    }

    fn parameter_check(&self, mut parameter: util::statistic::Parameter)-> Option<Parameter> {
        if parameter.datagram_size > crate::MAX_UDP_DATAGRAM_SIZE {
            error!("UDP datagram size is too big! Maximum is {}", crate::MAX_UDP_DATAGRAM_SIZE);
            return None;
        }

        if parameter.mode == util::NPerfMode::Sender && self.multiplex_port_receiver == MultiplexPort::Sharding && (self.multiplex_port == MultiplexPort::Sharing || self.multiplex_port == MultiplexPort::Sharding ) {
            warn!("Sharding on receiver side doesn't work, if sender side is set to sharing or sharding (uses one port), since all traffic would be balanced to one thread (see man for SO_REUSEPORT)!");
        }

        if parameter.mode == util::NPerfMode::Receiver && self.multiplex_port != MultiplexPort::Individual {
            warn!("Can't set sender multiplexing on receiver side!");
        }

        let cores_amount = core_affinity::get_core_ids().unwrap_or_default().len();
        if parameter.amount_threads > cores_amount as u16 {
            warn!("Amount of threads is bigger than available cores! Multiple threads are going to run on the same core! Available cores: {}", cores_amount);
        } else if parameter.amount_threads * 2 > cores_amount as u16 {
            warn!("If receiver/sender is running on the same machine, with the same amount of threads, multiple threads are going to run on the same core! Available cores: {}", cores_amount);
        }

        if parameter.mode == util::NPerfMode::Receiver && self.time != crate::DEFAULT_DURATION {
            warn!("Time is ignored in receiver mode!");
        }

        if parameter.io_model != IOModel::IoUring && (self.uring_mode != UringMode::Normal || self.uring_ring_size != crate::DEFAULT_URING_RING_SIZE) {
            warn!("Uring specific parameters are only used with io-model io_uring enabled!");
        }

        if !self.uring_ring_size.is_power_of_two() {
            error!("Uring ring size must be a power of 2!");
            return None;
        }

        if parameter.uring_parameter.burst_size > self.uring_ring_size {
            error!("Uring burst size {} must be smaller than the ring size {}!", parameter.uring_parameter.burst_size, self.uring_ring_size);
            return None;
        }

        if self.uring_ring_size > crate::URING_MAX_RING_SIZE {
            error!("Uring ring size is too big! Maximum is {}", crate::URING_MAX_RING_SIZE);
            return None;
        }

        if self.io_model == IOModel::IoUring && self.uring_mode == UringMode::Zerocopy && parameter.mode != util::NPerfMode::Sender {
            warn!("Zero copy is only available with io_uring on the sender!");
            return None;
        }

        if self.interval > 0.0 && (self.interval * (self.time as f64 / self.interval).round() - self.time as f64).abs() > 1e-9  {
            error!("Interval doesn't fit perfect in the time!");
            return None;
        }

        if self.interval > 0.0 && self.time == 0 && self.mode == NPerfMode::Receiver {
            error!("Interval is set but time is 0! Time must be set when interval output is enabled!");
            return None;
        }

        if Self::has_more_than_one_decimal(self.interval) {
            error!("Interval has more than one decimal place! Only tenth of a second is allowed!");
            return None;
        }

        // Convert Mbit/s to byte/s
        if ((self.with_socket_pacing_rate / 8 ) as u64 ) * 1024 * 1024 >= u32::MAX.into() {
            error!("Socket pacing rate is too big! Maximum is {} Mbit/s", u32::MAX / 1024 / 1024 * 8);
            return None;
        } else if self.with_socket_pacing_rate > 0 {
            parameter.socket_options.socket_pacing_rate = (self.with_socket_pacing_rate / 8) * 1024 * 1024;
        }

        if self.io_model == IOModel::IoUring && self.uring_mode == UringMode::Normal || self.uring_mode == UringMode::Zerocopy {
            warn!("Setting packet_buffer_size to {}!", parameter.uring_parameter.buffer_size);
            parameter.packet_buffer_size = parameter.uring_parameter.buffer_size as usize;
        }

        if self.uring_sqpoll_shared && !self.uring_sqpoll {
            warn!("Uring sqpoll_shared can't be used without sqpoll!");
            warn!("Setting sqpoll to true!");
            parameter.uring_parameter.sqpoll = true;
        }

        if parameter.uring_parameter.sqpoll && parameter.uring_parameter.task_work != UringTaskWork::Default {
            warn!("Neither DEFER nor COOP can be used with SQ_POLL! Setting task_work to Default!");
            parameter.uring_parameter.task_work = UringTaskWork::Default;
        }

        if parameter.output_file_path != path::PathBuf::from(crate::DEFAULT_FILE_NAME) {
            parameter.output_format = OutputFormat::File;
        }

        Some(parameter)
    }

    fn has_more_than_one_decimal(n: f64) -> bool {
        let s = format!("{}", n);
        let parts: Vec<&str> = s.split('.').collect();
        if parts.get(1).is_none() {
            return false;
        }
        parts[1].len() > 1
    }


    fn parse_socket_options(&self, mode: NPerfMode) -> SocketOptions {
        let gso = if self.with_gsro && mode == util::NPerfMode::Sender {
            Some(self.datagram_size)
        } else {
            None
        };

        let (recv_buffer_size, send_buffer_size) = if self.with_socket_buffer {
            info!("Setting udp buffer sizes with recv {} and send {}", crate::MAX_SOCKET_RECEIVE_BUFFER_SIZE, crate::MAX_SOCKET_SEND_BUFFER_SIZE);
            (Some(crate::MAX_SOCKET_RECEIVE_BUFFER_SIZE), Some(crate::MAX_SOCKET_SEND_BUFFER_SIZE))
        } else {
            info!("Setting buffer size of UDP socket disabled!");
            (Some(crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE), Some(crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE))
        };

        let gro = mode == util::NPerfMode::Receiver && self.with_gsro;

        let reuseport = match mode {
            NPerfMode::Sender => self.multiplex_port == MultiplexPort::Sharding,
            NPerfMode::Receiver => self.multiplex_port_receiver == MultiplexPort::Sharding,
        };
        
        SocketOptions::new(
            !self.without_non_blocking, 
            self.with_ip_frag, 
            reuseport,
            gso, 
            gro, 
            self.with_socket_pacing_rate,
            recv_buffer_size, 
            send_buffer_size
        )
    }
}
