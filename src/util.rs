use std::net::Ipv4Addr;

use log::{debug, info};

#[derive(PartialEq)]
pub enum NPerfMode {
    Client,
    Server,
}

pub struct NperfMeasurement<'a> {
    pub mode: NPerfMode,
    pub ip: Ipv4Addr,
    pub local_port: u16,
    pub remote_port: u16,
    pub buffer: &'a mut [u8; crate::DEFAULT_UDP_BLKSIZE],
    pub socket: i32,
    pub data_rate: u64,
    pub first_packet_received: bool,
    pub start_time: std::time::Instant,
    pub end_time: std::time::Instant,
    pub packet_count: u64,
    pub omitted_packet_count: u64,
}

#[derive(Debug)]
pub struct NperfHistory {
    pub total_time: std::time::Duration,
    pub total_data: u64,
    pub data_rate: f64,
    pub packet_loss: f64,
}

pub fn parse_mode(mode: String) -> Option<NPerfMode> {
    match mode.as_str() {
        "client" => Some(NPerfMode::Client),
        "server" => Some(NPerfMode::Server),
        _ => None,
    }
}


// Similar to iperf3's fill_with_repeating_pattern
pub fn fill_buffer_with_repeating_pattern(buffer: &mut [u8]) {
    let mut counter: u8 = 0;
    for i in buffer.iter_mut() {
        *i = (48 + counter).to_ascii_lowercase();

        if counter > 9 {
            counter = 0;
        } else {
            counter += 1;
        }
    }

    debug!("Filled buffer with repeating pattern {:?}", buffer);
}

pub fn create_history(measurement: &mut NperfMeasurement) -> NperfHistory {
    NperfHistory {
        total_time: calculate_total_time(measurement),
        total_data: calculate_total_data(measurement),
        data_rate: calculate_data_rate(measurement),
        packet_loss: calculate_packet_loss(measurement),
    }
}

fn calculate_total_data(measurement: &mut NperfMeasurement) -> u64 {
    let total_data = (measurement.packet_count * crate::DEFAULT_UDP_BLKSIZE as u64) / (1024 * 1024 * 1024);
    info!("Total data: {:.2} GBytes", total_data);
    total_data
}

fn calculate_data_rate(measurement: &mut NperfMeasurement) -> f64{
    let elapsed_time = measurement.end_time - measurement.start_time;
    let elapsed_time_in_seconds = elapsed_time.as_secs_f64();
    let data_rate = ((measurement.packet_count as f64 * crate::DEFAULT_UDP_BLKSIZE as f64) / (1024 * 1024 * 1024) as f64) / elapsed_time_in_seconds;
    info!("Data rate: {:.2} GBytes/s", data_rate);
    data_rate
}

fn calculate_packet_loss(measurement: &mut NperfMeasurement) -> f64 {
    let packet_loss = (measurement.omitted_packet_count as f64 / measurement.packet_count as f64) * 100.0;
    info!("Packet loss: {:.2}%", packet_loss);
    packet_loss
}

fn calculate_total_time(measurement: &mut NperfMeasurement) -> std::time::Duration {
    let total_time = measurement.end_time - measurement.start_time;
    info!("Total time: {:.2}s", total_time.as_secs_f64());
    total_time
}
