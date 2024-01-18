use std::{net::Ipv4Addr, time::Duration};
use log::{debug, info};

use crate::net::socket::Socket;

#[derive(PartialEq, Debug)]
pub enum NPerfMode {
    Client,
    Server,
}

#[derive(Debug)]
pub struct NperfMeasurement {
    pub mode: NPerfMode,
    pub run_infinite: bool,
    pub ip: Ipv4Addr,
    pub local_port: u16,
    pub remote_port: u16,
    pub buffer: Vec<u8>,
    pub dynamic_buffer_size: bool,
    pub socket: i32,
    pub time: u64,
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


pub fn prepare_packet(next_packet_id: u64, buffer: &mut Vec<u8>) {
    buffer[0..8].copy_from_slice(&next_packet_id.to_be_bytes());
    debug!("Prepared packet number: {}", u64::from_be_bytes(buffer[0..8].try_into().unwrap()));
}

// Packet reordering taken from iperf3 and rperf https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/stream/udp.rs#L225
// https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/stream/udp.rs#L225 
pub fn process_packet(buffer: &mut [u8], next_packet_id: u64, history: &mut History) -> u64 {
    let packet_id = u64::from_be_bytes(buffer[0..8].try_into().unwrap());
    debug!("Received packet number: {}", packet_id);

    if packet_id == next_packet_id {
        return 1
    } else if packet_id > next_packet_id {
        let lost_packet_count = (packet_id - next_packet_id) as u64;
        history.amount_omitted_datagrams += lost_packet_count as i64;
        info!("Reordered or lost packet received! Expected number {}, but received {}. {} packets are currently missing", next_packet_id, packet_id, lost_packet_count);
        return lost_packet_count + 1; // This is the next packet id that we expect, since we assume that the missing packets are lost
    } else { // If the received packet_id is smaller than the expected, it means that we received a reordered (or duplicated) packet.
        if history.amount_omitted_datagrams > 0 { 
            history.amount_omitted_datagrams -= 1;
            history.amount_reordered_datagrams  += 1;
            info!("Received reordered packet number {}, but expected {}", packet_id, next_packet_id);
        } else { 
            history.amount_duplicated_datagrams += 1;
            info!("Received duplicated packet");
        }
        return 0
    }
}

pub fn create_buffer_dynamic(socket: &Socket) -> Vec<u8> {
    let buffer_len = socket.get_mtu().expect("Error getting dynamically the socket MTU") as usize;
    info!("UDP MTU of size {} bytes", buffer_len);
    let buffer: Vec<u8> = vec![0; buffer_len];
    buffer
}

#[derive(Debug)]
pub struct History {
    pub start_time: std::time::Instant,
    pub end_time: std::time::Instant,
    total_time: std::time::Duration,
    total_data: f64,
    datagram_size: u64,
    pub amount_datagrams: u64,
    pub amount_reordered_datagrams: u64,
    pub amount_duplicated_datagrams: u64,
    pub amount_omitted_datagrams: i64,
    data_rate: f64,
    packet_loss: f64,
}

impl History {
    pub fn new(datagram_size: u64) -> History {
        History {
            start_time: std::time::Instant::now(),
            end_time: std::time::Instant::now(),
            total_time: Duration::new(0, 0),
            total_data: 0.0,
            datagram_size,
            amount_datagrams: 0,
            amount_reordered_datagrams: 0,
            amount_duplicated_datagrams: 0,
            amount_omitted_datagrams: 0,
            data_rate: 0.0,
            packet_loss: 0.0,
        }
    }

    fn update(&mut self) {
        self.total_data = self.calculate_total_data();
        self.total_time = self.calculate_total_time();
        self.data_rate = self.calculate_data_rate();
        self.packet_loss = self.calculate_packet_loss();
    }

    pub fn print(&mut self) {
        self.update();
        info!("Total time: {:.2}s", self.total_time.as_secs_f64());
        info!("Total data: {:.2} GiBytes", self.total_data);
        info!("Amount of datagrams: {}", self.amount_datagrams);
        info!("Amount of reordered datagrams: {}", self.amount_reordered_datagrams);
        info!("Amount of duplicated datagrams: {}", self.amount_duplicated_datagrams);
        info!("Amount of omitted datagrams: {}", self.amount_omitted_datagrams);
        info!("Data rate: {:.2} GiBytes/s / {:.2} GiBit/s", self.data_rate, (self.data_rate * 8.0));
        info!("Packet loss: {:.2}%", self.packet_loss);
    }

    fn calculate_total_data(&self) -> f64 {
        let total_data = (self.amount_datagrams * self.datagram_size as u64) as f64 / 1024.0 / 1024.0 / 1024.0;
        total_data 
    }
    
    fn calculate_data_rate(&self) -> f64{
        let elapsed_time = self.end_time - self.start_time;
        let elapsed_time_in_seconds = elapsed_time.as_secs_f64();
        let data_rate = ((self.amount_datagrams as f64 * self.datagram_size as f64) / (1024 * 1024 * 1024) as f64) / elapsed_time_in_seconds;
        data_rate
    }
    
    fn calculate_packet_loss(&self) -> f64 {
        let packet_loss = (self.amount_omitted_datagrams as f64 / self.amount_datagrams as f64) * 100.0;
        packet_loss
    }
    
    fn calculate_total_time(&self) -> std::time::Duration {
        let total_time = self.end_time - self.start_time;
        total_time
    }
}
