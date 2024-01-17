use std::time::Instant;

use libc::close;
use log::{debug, error, info};

use crate::util;
use crate::net;

pub fn start_client(measurement: &mut util::NperfMeasurement) {
    info!("Current mode: client");
    // Fill the buffer
    util::fill_buffer_with_repeating_pattern(&mut measurement.buffer);

    match net::set_socket_send_buffer_size(measurement.socket, crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE) {
        Ok(_) => {},
        Err(x) => { error!("{x}"); panic!()},
    };

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

    if measurement.dynamic_buffer_size {
        measurement.buffer = util::create_buffer_dynamic(measurement.socket);
    }

    measurement.start_time = Instant::now();
    let buffer_length = measurement.buffer.len();

    for _ in 0..5000000 { // 1,4GB
        util::prepare_packet(measurement);

        match net::send(measurement.socket, &mut measurement.buffer, buffer_length) {
            Ok(_) => {
                measurement.packet_count += 1;
                debug!("Sent data to remote host");
            },
            Err("ECONNREFUSED") => {
                error!("Start the server first! Abort measurement...");
                unsafe { close(measurement.socket) }; 
                return;
            },
            Err(x) => { 
                error!("{x}"); 
                unsafe { close(measurement.socket) }; 
                panic!()},
        };
    }
    let mut last_message_buffer: [u8; crate::LAST_MESSAGE_SIZE as usize] = [0; crate::LAST_MESSAGE_SIZE as usize];
    match net::send(measurement.socket, &mut last_message_buffer, crate::LAST_MESSAGE_SIZE as usize) {
        Ok(_) => {
            measurement.end_time = Instant::now();
            debug!("Finished sending data to remote host");
            util::print_out_history(&util::create_history(measurement));
        },
        Err(x) => { 
            error!("{x}"); 
            unsafe { close(measurement.socket) }; 
            panic!()},
    };

    unsafe { close(measurement.socket) }; 
}
