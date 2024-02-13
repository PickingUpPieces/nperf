use std::{ops::Add, time::{Duration, Instant}};
use log::debug;
use serde::Serialize;
use serde_json;

use crate::net::socket_options::SocketOptions;


#[derive(Debug, Serialize, Copy, Clone)]
pub struct Statistic {
    parameter: Parameter,
    test_duration: std::time::Duration,
    total_data_gbyte: f64,
    pub amount_datagrams: u64,
    pub amount_data_bytes: usize,
    pub amount_reordered_datagrams: u64,
    pub amount_duplicated_datagrams: u64,
    pub amount_omitted_datagrams: i64,
    pub amount_syscalls: u64,
    pub amount_io_model_syscalls: u64,
    data_rate_gbit: f64,
    packet_loss: f64,
}

#[derive(Debug, Serialize, Copy, Clone)]
pub struct Parameter {
    pub mode: super::NPerfMode,
    pub ip: std::net::Ipv4Addr,
    pub enable_json_output: bool,
    pub io_model: super::IOModel,
    pub test_runtime_length: u64,
    pub mss: u32,
    pub datagram_size: u32,
    pub packet_buffer_size: usize,
    pub socket_options: SocketOptions,
    pub exchange_function: super::ExchangeFunction,
}

// Measurement is used to measure the time of a specific statistc. Type time::Instant cannot be serialized, so it is not included in the Statistic struct.
#[derive(Debug, Copy, Clone)]
pub struct Measurement {
    pub start_time: std::time::Instant,
    pub end_time: std::time::Instant,
    pub statistic: Statistic,
    pub first_packet_received: bool,
    pub last_packet_received: bool,
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
            amount_syscalls: 0,
            amount_io_model_syscalls: 0,
            data_rate_gbit: 0.0,
            packet_loss: 0.0,
        }
    }

    pub fn calculate_statistics(&mut self) {
        debug!("Updating statistic...");
        self.total_data_gbyte = self.calculate_total_data();
        self.data_rate_gbit = self.calculate_data_rate();
        self.packet_loss = self.calculate_packet_loss();
        debug!("Statistic updated: {:?}", self);
    }

    pub fn print(&mut self) {
        self.calculate_statistics();

        if self.parameter.enable_json_output {
            println!("{}", serde_json::to_string(&self).unwrap());
            return;
        }

        println!("------------------------");
        println!("Statistics");
        println!("------------------------");
        println!("Total time: {:.2}s", self.test_duration.as_secs_f64());
        println!("Total data: {:.2} GiBytes", self.total_data_gbyte);
        println!("Amount of datagrams: {}", self.amount_datagrams);
        println!("Amount of reordered datagrams: {}", self.amount_reordered_datagrams);
        println!("Amount of duplicated datagrams: {}", self.amount_duplicated_datagrams);
        println!("Amount of omitted datagrams: {}", self.amount_omitted_datagrams);
        println!("Amount of syscalls: {}", self.amount_syscalls);
        println!("Amount of IO model syscalls: {}", self.amount_io_model_syscalls);
        println!("Data rate: {:.2} GiBytes/s / {:.2} Gibit/s", self.data_rate_gbit / 8.0, self.data_rate_gbit);
        println!("Packet loss: {:.2}%", self.packet_loss);
        println!("------------------------");
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


impl Add for Statistic {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        // Check if one is zero, to avoid division by zero
        let data_rate_gbit = if self.data_rate_gbit == 0.0 {
            other.data_rate_gbit
        } else if other.data_rate_gbit == 0.0 {
            self.data_rate_gbit
        } else {
            ( self.data_rate_gbit + other.data_rate_gbit ) / 2.0 // Data rate is the average of both
        };

        // Check if one is zero, to avoid division by zero
        let packet_loss = if self.packet_loss == 0.0 {
            other.packet_loss
        } else if other.packet_loss == 0.0 {
            self.packet_loss
        } else {
            ( self.packet_loss + other.packet_loss ) / 2.0 // Average of packet loss
        };

        // Assumption is that both statistics have the same test duration. 
        // Check if one is zero, to avoid division by zero.
        // Alternativly, could be added to a list of test_durations, but then we would need to change the type in the struct.
        let test_duration = if self.test_duration.as_secs() == 0 {
            other.test_duration
        } else {
            self.test_duration
        };

        Statistic {
            parameter: self.parameter, // Assumption is that both statistics have the same test parameters
            test_duration, 
            total_data_gbyte: self.total_data_gbyte + other.total_data_gbyte,
            amount_datagrams: self.amount_datagrams + other.amount_datagrams,
            amount_data_bytes: self.amount_data_bytes + other.amount_data_bytes,
            amount_reordered_datagrams: self.amount_reordered_datagrams + other.amount_reordered_datagrams,
            amount_duplicated_datagrams: self.amount_duplicated_datagrams + other.amount_duplicated_datagrams,
            amount_omitted_datagrams: self.amount_omitted_datagrams + other.amount_omitted_datagrams,
            amount_syscalls: self.amount_syscalls + other.amount_syscalls,
            amount_io_model_syscalls: self.amount_io_model_syscalls + other.amount_io_model_syscalls,
            data_rate_gbit, 
            packet_loss,
        }
    }
}

impl Measurement {
    pub fn new(parameter: Parameter) -> Measurement {
        Measurement {
            start_time: Instant::now(),
            end_time: Instant::now(),
            statistic: Statistic::new(parameter),
            first_packet_received: false,
            last_packet_received: false,
        }
    }
}