use clap::Parser;
use log::{info, error, debug};

use crate::node::client::Client;
use crate::node::server::Server;
use crate::node::Node;
use crate::net::socket_options::SocketOptions;
use crate::util::ExchangeFunction;

mod node;
mod net;
mod util;

// const UDP_RATE: usize = (1024 * 1024) // /* 1 Mbps */
const DEFAULT_MSS: u32= 1472;
const DEFAULT_UDP_DATAGRAM_SIZE: u32 = 1472;
const DEFAULT_GSO_BUFFER_SIZE: u32= 65507;
const DEFAULT_SOCKET_SEND_BUFFER_SIZE: u32 = 26214400; // 25MB; // The buffer size will be doubled by the kernel to account for overhead. See man 7 socket
const DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE: u32 = 26214400; // 25MB; // The buffer size will be doubled by the kernel to account for overhead. See man 7 socket
const DEFAULT_DURATION: u64 = 10; // /* seconds */
const DEFAULT_PORT: u16 = 45001;

// /* Maximum datagram size UDP is (64K - 1) - IP and UDP header sizes */
const MAX_UDP_DATAGRAM_SIZE: u32 = 65535 - 8 - 20;
const LAST_MESSAGE_SIZE: usize = 100;

const DEFAULT_AMOUNT_MSG_WHEN_SENDMMSG: usize = 1024;

#[derive(Parser,Default,Debug)]
#[clap(version, about="A network performance measurement tool")]
struct Arguments{
    /// Mode of operation: client or server
    #[arg(default_value_t = String::from("server"))]
    mode: String,

    /// IP address to measure against/listen on
    #[arg(short = 'a', default_value_t = String::from("0.0.0.0"))]
    ip: String,

    //() Port number to measure against/listen on 
    #[arg(short, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Don't stop the node after the first measurement
    #[arg(short, long, default_value_t = true)]
    run_infinite: bool,

    /// Set length of single datagram (Without IP and UDP headers)
    #[arg(short = 'l', default_value_t = DEFAULT_UDP_DATAGRAM_SIZE)]
    datagram_size: u32,

    /// Time to run the test
    #[arg(short = 't', default_value_t = DEFAULT_DURATION)]
    time: u64,

    /// Enable GSO on sending socket
    #[arg(long, default_value_t = false)]
    with_gso: bool,

    /// Set GSO buffer size which overwrites the MSS by default if GSO/GRO is enabled
    #[arg(long, default_value_t = DEFAULT_GSO_BUFFER_SIZE)]
    with_gso_buffer: u32,

    /// Set transmit buffer size. Gets overwritten by GSO/GRO buffer size if GSO/GRO is enabled.
    #[arg(long, default_value_t = DEFAULT_MSS)]
    with_mss: u32,

    /// Enable GRO on receiving socket
    #[arg(long, default_value_t = false)]
    with_gro: bool,

    /// Disable fragmentation on sending socket
    #[arg(long, default_value_t = true)]
    without_ip_frag: bool,

    /// Use sendmsg/recvmsg method for sending data
    #[arg(long, default_value_t = false)]
    with_msg: bool,    

    /// Use sendmmsg/recvmmsg method for sending data
    #[arg(long, default_value_t = false)]
    with_mmsg: bool, 

    /// Amount of message packs of gso_buffers to send when using sendmmsg
    #[arg(long, default_value_t = DEFAULT_AMOUNT_MSG_WHEN_SENDMMSG)]
    with_mmsg_amount: usize,

    /// Enable non-blocking socket
    #[arg(long, default_value_t = true)]
    with_non_blocking: bool,
}

fn main() {
    env_logger::init();
    let args = Arguments::parse();
    debug!("{:?}", args);

    let mode: util::NPerfMode = match util::parse_mode(args.mode) {
        Some(x) => x,
        None => { error!("Invalid mode! Should be 'client' or 'server'"); panic!()},
    };

    let ipv4 = match net::parse_ipv4(args.ip) {
        Ok(x) => x,
        Err(_) => { error!("Invalid IPv4 address!"); panic!()},
    };

    if args.datagram_size > MAX_UDP_DATAGRAM_SIZE {
        error!("UDP datagram size is too big! Maximum is {}", MAX_UDP_DATAGRAM_SIZE);
        panic!();
    } else {
        info!("UDP datagram size used: {}", args.datagram_size);
    }

    let (exchange_function, packet_buffer_size) = if args.with_msg {
        (ExchangeFunction::Msg, 1)
    } else if args.with_mmsg {
        (ExchangeFunction::Mmsg, args.with_mmsg_amount)
    } else {
        (ExchangeFunction::Normal, 1)
    };
    
    let mss = if args.with_gso || args.with_gro {
        info!("GSO/GRO enabled with buffer size {}", args.with_gso_buffer);
        args.with_gso_buffer
    } else {
        args.with_mss
    };
    info!("MSS used: {}", mss);
    info!("Exchange function used: {:?}", exchange_function);


    loop {
        let socket_options = SocketOptions::new(args.with_non_blocking, args.without_ip_frag, (args.with_gso, args.datagram_size), args.with_gro, crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE, crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE);
        let mut node: Box<dyn Node> = if mode == util::NPerfMode::Client {
            Box::new(Client::new(ipv4, args.port, mss, args.datagram_size, packet_buffer_size, socket_options, args.time, exchange_function))
        } else {
            Box::new(Server::new(ipv4, args.port, mss, args.datagram_size, packet_buffer_size, socket_options, args.run_infinite, exchange_function))
        };

        match node.run() {
            Ok(_) => { 
                info!("Finished measurement!");
                if !(args.run_infinite && mode == util::NPerfMode::Server) {
                    break;
                }
            },
            Err(x) => {
                error!("Error running app: {}", x);
                break;
            }
        }
    }
}
