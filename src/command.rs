use clap::Parser;
use log::{error, info};

use crate::util::{self, statistic::{MultiplexPort, OutputFormat, SimulateConnection}, IOModel, NPerfMode, ExchangeFunction};
use crate::net::{self, socket_options::SocketOptions};

#[derive(Parser,Default,Debug)]
#[clap(version, about="A network performance measurement tool")]
#[allow(non_camel_case_types)]
pub struct nPerf {
    /// Mode of operation: client or server
    #[arg(default_value_t, value_enum)]
    mode: NPerfMode,

    /// IP address to measure against/listen on
    #[arg(short = 'a',long, default_value_t = String::from("0.0.0.0"))]
    ip: String,

    /// Port number to measure against, server listen on.
    #[arg(short, long, default_value_t = crate::DEFAULT_SERVER_PORT)]
    pub port: u16,

    /// Port number clients send from.
    #[arg(short, long, default_value_t = crate::DEFAULT_CLIENT_PORT)]
    pub client_port: u16,

    /// Start multiple client/server threads in parallel. The port number will be incremented automatically.
    #[arg(long, default_value_t = 1)]
    parallel: u16,

    /// Don't stop the node after the first measurement
    #[arg(short, long, default_value_t = false)]
    pub run_infinite: bool,

    /// Set length of single datagram (Without IP and UDP headers)
    #[arg(short = 'l', long, default_value_t = crate::DEFAULT_UDP_DATAGRAM_SIZE)]
    datagram_size: u32,

    /// Time to run the test
    #[arg(short = 't', long, default_value_t = crate::DEFAULT_DURATION)]
    time: u64,

    /// Enable GSO/GRO on socket
    #[arg(long, default_value_t = false)]
    with_gsro: bool,

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

    /// Define the data structure type the output 
    #[arg(long, default_value_t, value_enum)]
    output_format: OutputFormat,

    /// Use different port number for each client thread, share a single port or shard a single port with reuseport
    #[arg(long, default_value_t, value_enum)]
    multiplex_port: MultiplexPort,

    /// Same as for multiplex_port, but for the server
    #[arg(long, default_value_t, value_enum)]
    multiplex_port_server: MultiplexPort,

    /// CURRENTLY IGNORED. Simulate a single QUIC connection or one QUIC connection per thread.
    #[arg(long, default_value_t, value_enum)]
    simulate_connection: SimulateConnection,

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

        // Setting simulate_connection to the currently supported values -> quite ugly 
        let simulate_connection = match self.mode {
            NPerfMode::Client => {
                match self.multiplex_port_server {
                    MultiplexPort::Individual => SimulateConnection::Multiple,
                    _ => SimulateConnection::Single
                }
            },
            NPerfMode::Server => {
                if self.multiplex_port_server == MultiplexPort::Individual { SimulateConnection::Multiple } else { SimulateConnection::Single }
            }
        };

        info!("Simulate connection: {:?}", simulate_connection);
        info!("Exchange function used: {:?}", self.exchange_function);
        info!("MSS used: {}", mss);
        info!("IO model used: {:?}", self.io_model);
        info!("Output format: {:?}", self.output_format);
        info!("UDP datagram size used: {}", self.datagram_size);

        let socket_options = self.parse_socket_options(self.mode);


        let parameter = util::statistic::Parameter::new(
            self.mode, 
            ipv4, 
            self.parallel,
            self.output_format, 
            self.io_model, 
            self.time, 
            mss, 
            self.datagram_size, 
            packet_buffer_size, 
            socket_options, 
            self.exchange_function,
            self.multiplex_port,
            self.multiplex_port_server,
            simulate_connection
        );

        match self.parameter_check(&parameter) {
            false => { error!("Invalid parameter!"); None },
            true => { Some(parameter) }
        }
    }

    fn parameter_check(&self, parameter: &util::statistic::Parameter)-> bool {
        if parameter.datagram_size > crate::MAX_UDP_DATAGRAM_SIZE {
            error!("UDP datagram size is too big! Maximum is {}", crate::MAX_UDP_DATAGRAM_SIZE);
            return false;
        }

        if parameter.mode == util::NPerfMode::Client && self.multiplex_port_server == MultiplexPort::Sharding && (self.multiplex_port == MultiplexPort::Sharing || self.multiplex_port == MultiplexPort::Sharding ) {
            error!("Sharding on server side not available if client side is set to sharing or sharding (uses one port), since all traffic would be balanced to one thread (see man for SO_REUSEPORT)!");
            return false;
        }

        if parameter.mode == util::NPerfMode::Server && self.multiplex_port != MultiplexPort::Individual {
            error!("Can't set client multiplexing on server side!");
            return false;
        }

        true
    }

    fn parse_socket_options(&self, mode: NPerfMode) -> SocketOptions {
        let gso = if self.with_gsro && mode == util::NPerfMode::Client {
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

        let gro = mode == util::NPerfMode::Server && self.with_gsro;
        
        SocketOptions::new(
            !self.without_non_blocking, 
            self.with_ip_frag, 
            self.multiplex_port == MultiplexPort::Sharding,
            gso, 
            gro, 
            recv_buffer_size, 
            send_buffer_size
        )
    }
}
