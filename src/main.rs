use std::{time::Instant, vec};
use clap::Parser;
use log::{info, error};
use server::*;
use client::*;

mod util;
mod net;
mod client;
mod server;

// const UDP_RATE: usize = (1024 * 1024) // /* 1 Mbps */
const DEFAULT_UDP_BLKSIZE: usize = 1472;

const LAST_MESSAGE_SIZE: isize = 100;
const DEFAULT_SOCKET_SEND_BUFFER_SIZE: u32 = 26214400; // 25MB;
const DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE: u32 = 26214400; // 25MB;
// const DURATION: usize = 10 // /* seconds */

// Sanity checks from iPerf3
// /* Maximum size UDP send is (64K - 1) - IP and UDP header sizes */
const MAX_UDP_BLOCKSIZE: usize = 65535 - 8 - 20;

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
    #[arg(short, default_value_t = 45001)]
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
    #[arg(short = 't', default_value_t = 10)]
    time: u64,
}

fn main() {
    env_logger::init();
    let args = Arguments::parse();
    info!("{:?}", args);

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

    let mut measurement = util::NperfMeasurement {
        mode,
        run_infinite: args.run_server_infinite,
        ip: ipv4,
        local_port: args.port,
        remote_port: 0,
        buffer: vec![0; args.mtu_size],
        dynamic_buffer_size: args.mtu_discovery,
        socket: 0,
        time: args.time,
        data_rate: 0,
        first_packet_received: false,
        start_time: Instant::now(),
        end_time: Instant::now(),
        packet_count: 0,
        next_packet_id: 0,
        omitted_packet_count: 0,
        reordered_packet_count: 0,
        duplicated_packet_count: 0,
    };

    if measurement.mode == util::NPerfMode::Client {
        let client = client::new(measurement.ip, measurement.local_port, measurement.mtu_size, measurement.mtu_discovery, measurement.time);
        client.run();
    } else {
        let server = server::new(measurement.ip, measurement.local_port, measurement.mtu_size, measurement.mtu_discovery, measurement.run_infinite);
        server.run();
    }
}
