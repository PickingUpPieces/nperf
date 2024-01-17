
use std::time::Instant;

use libc::close;
use log::{debug, error, info};

use crate::util;
use crate::net;

pub fn start_server(measurement: &mut util::NperfMeasurement) {
    info!("Current mode: server");
    match net::bind_socket(measurement.socket, measurement.ip, measurement.local_port) {
        Ok(_) => info!("Bound socket to port: {}:{}", measurement.ip, measurement.local_port),
        Err(x) => { error!("{x}"); panic!()},
    };

    match net::set_socket_receive_buffer_size(measurement.socket, crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE) {
        Ok(_) => {},
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
        match net::recv(measurement.socket, &mut measurement.buffer) {
            Ok(amount_received_bytes) => {
                if measurement.first_packet_received == false {
                    measurement.first_packet_received = true;
                    info!("First packet received!");

                    if measurement.dynamic_buffer_size {
                        info!("Set buffer size to MTU");
                        measurement.buffer = util::create_buffer_dynamic(measurement.socket);
                    }

                    measurement.start_time = Instant::now();
                }

                if amount_received_bytes == crate::LAST_MESSAGE_SIZE {
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
    util::print_out_history(&util::create_history(measurement));

}