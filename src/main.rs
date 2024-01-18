use clap::Parser;
use log::{info, error, debug};

use crate::client::Client;
use crate::server::Server;
use crate::net::socket_options::SocketOptions;

mod util;
mod net;
mod client;
mod server;

// const UDP_RATE: usize = (1024 * 1024) // /* 1 Mbps */
const DEFAULT_UDP_BLKSIZE: usize = 1472;
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
    #[arg(short, default_value_t = String::from("server"))]
    mode: String,

    /// IP address to measure against/listen on
    #[arg(short = 'a', default_value_t = String::from("0.0.0.0"))]
    ip: String,

    //() Port number to measure against/listen on 
    #[arg(short, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Don't stop the server after the first measurement
    #[arg(short, long, default_value_t = false)]
    run_server_infinite: bool,

    /// Set MTU size (Without IP and UDP headers)
    #[arg(short = 'l', default_value_t = DEFAULT_UDP_BLKSIZE)]
    mtu_size: usize,

    /// Dynamic MTU size discovery
    #[arg(short = 'd', default_value_t = false)]
    mtu_discovery: bool,

    /// Time to run the test
    #[arg(short = 't', default_value_t = DEFAULT_DURATION)]
    time: u64,

    /// Enable GSO on sending socket
    #[arg(long, default_value_t = false)]
    use_gso: bool,
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

    let mut socket_options = SocketOptions::new(true, (args.use_gso, args.mtu_size as u64), (false, 0), crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE, crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE);

    if mode == util::NPerfMode::Client {
        let mut client = Client::new(ipv4, args.port, args.mtu_size, args.mtu_discovery, socket_options, args.time);
        client.run();
    } else {
        let mut server = Server::new(ipv4, args.port, args.mtu_size, args.mtu_discovery, socket_options, args.run_server_infinite);
        server.run();
    }
}
