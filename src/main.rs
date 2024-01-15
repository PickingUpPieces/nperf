use std::time::Instant;
use clap::Parser;
use libc::close;
use log::{info, error, debug};

use crate::util::prepare_packet;

mod util;
mod net;

// Defaults from iPerf3
// #define UDP_RATE (1024 * 1024) /* 1 Mbps */
const DEFAULT_UDP_BLKSIZE: usize = 1470;
const LAST_MESSAGE_SIZE: isize = 100;
// #define DURATION 10 /* seconds */

// Sanity checks from iPerf3
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

    #[arg(short, long, default_value_t = false)]
    run_infinite: bool,
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


    let mut measurement = util::NperfMeasurement {
        mode,
        run_infinite: args.run_infinite,
        ip: ipv4,
        local_port: args.port,
        remote_port: 0,
        buffer: &mut [0; DEFAULT_UDP_BLKSIZE],
        socket: 0,
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

    measurement.socket = match net::create_socket() {
        Ok(x) => x,
        Err(x) => panic!("{x}"),
    };

    if measurement.mode == util::NPerfMode::Client {
        start_client(&mut measurement);
    } else {
        start_server(&mut measurement);
    }
}

fn start_server(measurement: &mut util::NperfMeasurement) {
    info!("Current mode: server");
    match net::bind_socket(measurement.socket, measurement.ip, measurement.local_port) {
        Ok(_) => info!("Bound socket to port: {}:{}", measurement.ip, measurement.local_port),
        Err(x) => { error!("{x}"); panic!()},
    };


    match net::set_socket_nonblocking(measurement.socket) {
        Ok(_) => info!("Set socket to non-blocking"),
        Err(x) => { 
                error!("{x}"); 
                unsafe { close(measurement.socket) }; 
                panic!()},
    };

    loop {
        match net::recv(measurement.socket, measurement.buffer) {
            Ok(amount_received_bytes) => {
                if measurement.first_packet_received == false {
                    measurement.first_packet_received = true;
                    info!("First packet received!");
                    measurement.start_time = Instant::now();
                }

                if amount_received_bytes == LAST_MESSAGE_SIZE {
                    info!("Last packet received!");
                    break;
                }
                util::process_packet(measurement);
                measurement.packet_count += 1;
            },
            Err(x) => {
                if x == "EAGAIN" {
                    continue;
                } else {
                    error!("{x}"); 
                    unsafe { close(measurement.socket) }; 
                    panic!();
                }
            }
        };

    }

    measurement.end_time = Instant::now();
    debug!("Finished receiving data from remote host");
    info!("{:?}", util::create_history(measurement));

}

fn start_client(measurement: &mut util::NperfMeasurement) {
    info!("Current mode: client");
    // Fill the buffer
    util::fill_buffer_with_repeating_pattern(measurement.buffer);

    match net::connect(measurement.socket, measurement.ip, measurement.local_port) {
        Ok(_) => info!("Connected to remote host: {}:{}", measurement.ip, measurement.local_port),
        Err(x) => { 
                error!("{x}"); 
                unsafe { close(measurement.socket) }; 
                panic!()},
    };

    match net::set_socket_nonblocking(measurement.socket) {
        Ok(_) => info!("Set socket to non-blocking"),
        Err(x) => { 
                error!("{x}"); 
                unsafe { close(measurement.socket) }; 
                panic!()},
    };

    measurement.start_time = Instant::now();

    for _ in 0..1000000 { // 1,4GB
        prepare_packet(measurement);

        match net::send(measurement.socket, measurement.buffer) {
            Ok(_) => {
                measurement.packet_count += 1;
                debug!("Sent data to remote host");
            },
            Err(x) => { 
                error!("{x}"); 
                unsafe { close(measurement.socket) }; 
                panic!()},
        };
    }

    match net::send(measurement.socket, measurement.buffer[0..LAST_MESSAGE_SIZE as usize].as_mut()) {
        Ok(_) => {
            measurement.end_time = Instant::now();
            debug!("Finished sending data to remote host");
            info!("{:?}", util::create_history(&measurement));
        },
        Err(x) => { 
            error!("{x}"); 
            unsafe { close(measurement.socket) }; 
            panic!()},
    };

    unsafe { close(measurement.socket) }; 
}
