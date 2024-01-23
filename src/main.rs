use clap::Parser;
use log::{info, error, debug};

use crate::node::client::Client;
use crate::node::server::Server;
use crate::node::Node;
use crate::net::socket_options::SocketOptions;

mod node;
mod util;
mod net;

// const UDP_RATE: usize = (1024 * 1024) // /* 1 Mbps */
const DEFAULT_UDP_BLKSIZE: usize = 1472;
const DEFAULT_GSO_BUFFER_SIZE: usize = 65507;
const DEFAULT_SOCKET_SEND_BUFFER_SIZE: u32 = 26214400; // 25MB;
const DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE: u32 = 26214400; // 25MB;
const DEFAULT_DURATION: u64 = 10; // /* seconds */
const DEFAULT_PORT: u16 = 45001;

// Sanity checks from iPerf3
// /* Maximum size UDP send is (64K - 1) - IP and UDP header sizes */
const MAX_UDP_BLOCKSIZE: usize = 65535 - 8 - 20;
const LAST_MESSAGE_SIZE: isize = 100;

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
    #[arg(short, long, default_value_t = false)]
    run_infinite: bool,

    /// Set MTU/gso-size size (Without IP and UDP headers)
    #[arg(short = 'l', default_value_t = DEFAULT_UDP_BLKSIZE)]
    mtu_size: usize,

    /// Dynamic MTU size discovery
    #[arg(long, default_value_t = false)]
    mtu_discovery: bool,

    /// Time to run the test
    #[arg(short = 't', default_value_t = DEFAULT_DURATION)]
    time: u64,

    /// Enable GSO on sending socket
    #[arg(long, default_value_t = false)]
    with_gso: bool,

    /// Set buffer transmit size
    #[arg(long, default_value_t = DEFAULT_GSO_BUFFER_SIZE)]
    with_gso_buffer_size: usize,

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

    if args.mtu_size > MAX_UDP_BLOCKSIZE {
        error!("MTU size is too big! Maximum is {}", MAX_UDP_BLOCKSIZE);
        panic!();
    } else {
        info!("MTU size used: {}", args.mtu_size);
    }

    let exchange_function = if args.with_msg {
        util::ExchangeFunction::Msg
    } else {
        if args.with_mmsg {
            util::ExchangeFunction::Mmsg
        } else {
            util::ExchangeFunction::Normal
        }
    };
    
    let mtu = if args.with_gso || args.with_gro {
        info!("GSO/GRO enabled with buffer size {}", args.with_gso_buffer_size);
        args.with_gso_buffer_size
    } else {
        args.mtu_size
    };
    info!("Exchange function used: {:?}", exchange_function);

    let socket_options = SocketOptions::new(args.with_non_blocking, args.without_ip_frag, (args.with_gso, args.mtu_size as u32), (args.with_gro, args.mtu_size as u32), crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE, crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE);

    let mut node: Box<dyn Node> = if mode == util::NPerfMode::Client {
        Box::new(Client::new(ipv4, args.port, mtu, args.mtu_discovery, socket_options, args.time, exchange_function))
    } else {
        Box::new(Server::new(ipv4, args.port, mtu, args.mtu_discovery, socket_options, args.run_infinite, exchange_function))
    };

    match node.run() {
        Ok(_) => info!("Finished measurement!"),
        Err(x) => {
            error!("Error running app: {}", x);
        }
    }
}
