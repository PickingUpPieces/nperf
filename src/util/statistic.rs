use std::time::Duration;
use log::debug;
use serde::Serialize;
use serde_json;

use crate::net::socket_options::SocketOptions;

#[derive(Debug, Serialize)]
pub struct Statistic {
    parameter: Parameter,
    test_duration: std::time::Duration,
    total_data_gbyte: f64,
    pub amount_datagrams: u64,
    pub amount_data_bytes: usize,
    pub amount_reordered_datagrams: u64,
    pub amount_duplicated_datagrams: u64,
    pub amount_omitted_datagrams: i64,
    data_rate_gbit: f64,
    packet_loss: f64,
}

#[derive(Debug, Serialize, Copy, Clone)]
pub struct Parameter {
    pub mode: super::NPerfMode,
    pub enable_json_output: bool,
    pub io_model: super::IOModel,
    pub test_runtime_length: u64,
    pub mss: u32,
    pub datagram_size: u32,
    pub packet_buffer_size: usize,
    pub socket_options: SocketOptions,
    pub exchange_function: super::ExchangeFunction,
}

impl Statistic {
    pub fn new(parameter: Parameter) -> Statistic {
        Statistic {
            parameter,
            test_duration: Duration::new(0, 0),
            total_data_gbyte: 0.0,
            amount_datagrams: 0,
            amount_data_bytes: 0,
            amount_reordered_datagrams: 0,
            amount_duplicated_datagrams: 0,
            amount_omitted_datagrams: 0,
            data_rate_gbit: 0.0,
            packet_loss: 0.0,
        }
    }

    fn update(&mut self) {
        debug!("Updating statistic...");
        self.total_data_gbyte = self.calculate_total_data();
        self.data_rate_gbit = self.calculate_data_rate();
        self.packet_loss = self.calculate_packet_loss();
        debug!("Statistic updated: {:?}", self);
    }

    pub fn print(&mut self) {
        self.update();

        if self.parameter.enable_json_output {
            println!("{}", serde_json::to_string(&self).unwrap());
            return;
        }

        println!("Total time: {:.2}s", self.test_duration.as_secs_f64());
        println!("Total data: {:.2} GiBytes", self.total_data_gbyte);
        println!("Amount of datagrams: {}", self.amount_datagrams);
        println!("Amount of reordered datagrams: {}", self.amount_reordered_datagrams);
        println!("Amount of duplicated datagrams: {}", self.amount_duplicated_datagrams);
        println!("Amount of omitted datagrams: {}", self.amount_omitted_datagrams);
        println!("Data rate: {:.2} GiBytes/s / {:.2} Gibit/s", self.data_rate_gbit / 8.0, self.data_rate_gbit);
        println!("Packet loss: {:.2}%", self.packet_loss);
    }

    fn calculate_total_data(&self) -> f64 {
        self.amount_data_bytes as f64 / 1024.0 / 1024.0 / 1024.0
    }
    
    fn calculate_data_rate(&self) -> f64{
        let elapsed_time_in_seconds = self.test_duration.as_secs_f64();
        ( self.total_data_gbyte / elapsed_time_in_seconds ) * 8.0
    }
    
    fn calculate_packet_loss(&self) -> f64 {
        (self.amount_omitted_datagrams as f64 / self.amount_datagrams as f64) * 100.0
    }
    
    pub fn set_test_duration(&mut self, start_time: std::time::Instant, end_time: std::time::Instant) {
        self.test_duration = end_time - start_time
    }
}
