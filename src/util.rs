use std::net::Ipv4Addr;

use log::{debug, info};

#[derive(PartialEq, Debug)]
pub enum NPerfMode {
    Client,
    Server,
}

#[derive(Debug)]
pub struct NperfMeasurement<'a> {
    pub mode: NPerfMode,
    pub run_infinite: bool,
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
    pub next_packet_id: u64,
    pub omitted_packet_count: i64,
    pub reordered_packet_count: u64,
    pub duplicated_packet_count: u64,
}

#[derive(Debug)]
pub struct NperfHistory {
    pub total_time: std::time::Duration,
    pub total_data: f64,
    pub amount_datagrams: u64,
    pub amount_reordered_datagrams: u64,
    pub amount_duplicated_datagrams: u64,
    pub amount_omitted_datagrams: i64,
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

pub fn create_history(measurement: &NperfMeasurement) -> NperfHistory {
    NperfHistory {
        total_time: calculate_total_time(measurement),
        total_data: calculate_total_data(measurement),
        amount_datagrams: measurement.packet_count,
        amount_reordered_datagrams: measurement.reordered_packet_count,
        amount_duplicated_datagrams: measurement.duplicated_packet_count,
        amount_omitted_datagrams: measurement.omitted_packet_count,
        data_rate: calculate_data_rate(measurement),
        packet_loss: calculate_packet_loss(measurement),
    }
}

fn calculate_total_data(measurement: &NperfMeasurement) -> f64 {
    let total_data = (measurement.packet_count * crate::DEFAULT_UDP_BLKSIZE as u64) as f64 / 1024.0 / 1024.0 / 1024.0;
    info!("Total data: {:.2} GiBytes", total_data);
    total_data 
}

fn calculate_data_rate(measurement: &NperfMeasurement) -> f64{
    let elapsed_time = measurement.end_time - measurement.start_time;
    let elapsed_time_in_seconds = elapsed_time.as_secs_f64();
    let data_rate = ((measurement.packet_count as f64 * crate::DEFAULT_UDP_BLKSIZE as f64) / (1024 * 1024 * 1024) as f64) / elapsed_time_in_seconds;
    info!("Data rate: {:.2} GiBytes/s / {:.2} GiBit/s", data_rate, (data_rate * 8.0));
    data_rate
}

fn calculate_packet_loss(measurement: &NperfMeasurement) -> f64 {
    let packet_loss = (measurement.omitted_packet_count as f64 / measurement.packet_count as f64) * 100.0;
    info!("Packet loss: {:.2}%", packet_loss);
    packet_loss
}

fn calculate_total_time(measurement: &NperfMeasurement) -> std::time::Duration {
    let total_time = measurement.end_time - measurement.start_time;
    info!("Total time: {:.2}s", total_time.as_secs_f64());
    total_time
}

pub fn prepare_packet(measurement: &mut NperfMeasurement) {
    measurement.buffer[0..8].copy_from_slice(&measurement.packet_count.to_be_bytes());
    debug!("Prepared packet number: {}", u64::from_be_bytes(measurement.buffer[0..8].try_into().unwrap()));
}

// Packet reordering taken from iperf3 and rperf https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/stream/udp.rs#L225
// https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/stream/udp.rs#L225 
pub fn process_packet(measurement: &mut NperfMeasurement) {
    let packet_id = u64::from_be_bytes(measurement.buffer[0..8].try_into().unwrap());
    debug!("Received packet number: {}", packet_id);

    if packet_id == measurement.next_packet_id {
        measurement.next_packet_id += 1;
    } else if packet_id > measurement.next_packet_id {
        let lost_packet_count = (packet_id - measurement.next_packet_id) as i64;
        measurement.omitted_packet_count += lost_packet_count;
        measurement.next_packet_id = packet_id + 1; 
        info!("Reordered or lost packet received! {} packets are currently missing", lost_packet_count);
    } else {
        if measurement.omitted_packet_count > 0 { 
            measurement.omitted_packet_count -= 1;
            measurement.reordered_packet_count += 1;
            info!("Received reordered packet");
        } else { 
            measurement.duplicated_packet_count += 1;
            info!("Received duplicated packet");
        }
    }
}
