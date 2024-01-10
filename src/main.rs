
use std::{result, thread::sleep};

use clap::Parser;
use log::{info, error};

mod util;
mod net;

// Defaults from iPerf3
// #define UDP_RATE (1024 * 1024) /* 1 Mbps */
// #define DEFAULT_UDP_BLKSIZE 1460 /* default is dynamically set, else this */
const DEFAULT_UDP_BLKSIZE: usize = 1460;
// #define DURATION 10 /* seconds */
/* Minimum size UDP send is the size of two 32-bit ints followed by a 64-bit int */
// #define MIN_UDP_BLOCKSIZE (4 + 4 + 8)
// /* Maximum size UDP send is (64K - 1) - IP and UDP header sizes */
// #define MAX_UDP_BLOCKSIZE (65535 - 8 - 20)

#[derive(Parser,Default,Debug)]
#[clap(version, about="A network performance measurement tool")]
struct Arguments{
    // Mode of operation: client or server
    #[arg(default_value_t = String::from("server"))]
    mode: String,
    // IP address to measure against/listen on
    #[arg(default_value_t = String::from("0.0.0.0"))]
    ip: String,
    // Port number to measure against/listen on 
    #[arg(default_value_t = 45001)]
    port: u16,
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

    let mut new_measurement = util::NperfMeasurement {
        mode,
        ip: ipv4,
        local_port: args.port,
        remote_port: 0,
        buffer: &mut [0; 1460],
        socket: 0,
        data_rate: 0,
        packet_count: 0,
        omitted_packet_count: 0,
    };

    new_measurement.socket = match net::create_socket() {
        Ok(x) => x,
        Err(x) => panic!("{x}"),
    };

    if new_measurement.mode == util::NPerfMode::Client {
        start_client(new_measurement);
    } else {
        start_server(new_measurement);
    }
}

fn start_server(new_measurement: util::NperfMeasurement) {
    info!("Current mode: server");
    match net::bind_socket(new_measurement.socket, new_measurement.ip, new_measurement.local_port) {
        Ok(_) => info!("Bound socket to port: {}:{}", new_measurement.ip, new_measurement.local_port),
        Err(x) => { error!("{x}"); panic!()},
    };

    loop {
        match net::recv(new_measurement.socket, new_measurement.buffer) {
            Ok(_) => info!("Received data from remote host"),
            Err(x) => { error!("{x}"); panic!()},
        };
    }
}

fn start_client(new_measurement: util::NperfMeasurement) {
    info!("Current mode: client");
    // Fill the buffer
    util::fill_buffer_with_repeating_pattern(new_measurement.buffer);

    match net::connect(new_measurement.socket, new_measurement.ip, new_measurement.local_port) {
        Ok(_) => info!("Connected to remote host: {}:{}", new_measurement.ip, new_measurement.local_port),
        Err(x) => { error!("{x}"); panic!()},
    };

    loop {
        match net::send(new_measurement.socket, new_measurement.buffer) {
            Ok(_) => info!("Sent data to remote host"),
            Err(x) => { error!("{x}"); panic!()},
        };
        sleep(std::time::Duration::from_secs(1));
    }
}
