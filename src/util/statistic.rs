use std::{ops::Add, time::{Duration, Instant}};
use log::debug;
use serde::Serialize;
use serde_json;
use crate::net::socket_options::SocketOptions;
use serde::{Deserialize, Deserializer, Serializer};
use std::collections::HashMap;


#[derive(clap::ValueEnum, Default, PartialEq, Debug, Clone, Copy, Serialize)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(clap::ValueEnum, Debug, PartialEq, Serialize, Clone, Copy, Default)]
pub enum MultiplexPort {
    #[default]
    Individual,
    Sharing,
    Sharding
}

#[derive(clap::ValueEnum, Debug, PartialEq, Serialize, Clone, Copy, Default)]
pub enum SimulateConnection {
    Single,
    #[default]
    Multiple
}

#[derive(clap::ValueEnum, Debug, PartialEq, Serialize, Clone, Copy, Default)]
pub enum UringSqFillingMode {
    #[default]
    Topup,
    Burst,
    Syscall 
}

#[derive(Debug, Serialize, Clone)]
pub struct Statistic {
    pub parameter: Parameter,
    pub test_duration: std::time::Duration,
    pub total_data_gbyte: f64,
    pub amount_datagrams: u64,
    pub amount_data_bytes: usize,
    pub amount_reordered_datagrams: u64,
    pub amount_duplicated_datagrams: u64,
    pub amount_omitted_datagrams: i64,
    pub amount_syscalls: u64,
    pub amount_io_model_calls: u64,
    pub data_rate_gbit: f64,
    pub packet_loss: f64,
    pub uring_canceled_multishot: u64,
    #[serde(with = "utilization")]
    pub uring_cq_utilization: Box<[usize]>,
    #[serde(with = "utilization")]
    pub uring_inflight_utilization: Box<[usize]>,
}


// Measurement is used to measure the time of a specific statistc. Type time::Instant cannot be serialized, so it is not included in the Statistic struct.
#[derive(Debug, Clone)]
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
            amount_io_model_calls: 0,
            data_rate_gbit: 0.0,
            packet_loss: 0.0,
            uring_canceled_multishot: 0,
            uring_cq_utilization: vec![0_usize; (crate::URING_MAX_RING_SIZE * 2) as usize].into_boxed_slice(),
            uring_inflight_utilization: vec![0_usize; (crate::URING_MAX_RING_SIZE * crate::URING_BUFFER_SIZE_MULTIPLICATOR) as usize].into_boxed_slice()
        }
    }

    pub fn calculate_statistics(&mut self) {
        debug!("Updating statistic...");
        self.total_data_gbyte = self.calculate_total_data();
        self.data_rate_gbit = self.calculate_data_rate();
        self.packet_loss = self.calculate_packet_loss();
        debug!("Statistic updated: {:?}", self);
    }

    pub fn print(&mut self, output_format: OutputFormat) {
        self.calculate_statistics();

        match output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(&self).unwrap());
            },
            OutputFormat::Text => {
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
                println!("Amount of IO model syscalls: {}", self.amount_io_model_calls);
                println!("Data rate: {:.2} GiBytes/s / {:.2} Gibit/s", self.data_rate_gbit / 8.0, self.data_rate_gbit);
                println!("Packet loss: {:.2}%", self.packet_loss);
                println!("------------------------");
                if self.parameter.io_model == super::IOModel::IoUring {
                    println!("Uring canceled multishot: {}", self.uring_canceled_multishot);
                    println!("Uring CQ utilization:");
                    // Print out an enumerate table with the utilization of the CQ and inflight count; Leave out all zero values
                    for (index, &utilization) in self.uring_cq_utilization.iter().enumerate() {
                        if utilization != 0 && utilization != 1 {
                            println!("CQ[{}]: {}", index, utilization);
                        }
                    }
                    for(index, &utilization) in self.uring_inflight_utilization.iter().enumerate() {
                        if utilization != 0 && utilization != 1 {
                            println!("Inflight[{}]: {}", index, utilization);
                        }
                    }
                }
            }
        }
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

        // Add the arrays field by field
        let mut uring_cq_utilization = vec![0; (crate::URING_MAX_RING_SIZE * 2) as usize].into_boxed_slice();
        for i in 0..uring_cq_utilization.len() {
            uring_cq_utilization[i] = self.uring_cq_utilization[i] + other.uring_cq_utilization[i];
        }

        let mut uring_inflight_utilization = vec![0; (crate::URING_MAX_RING_SIZE * crate::URING_BUFFER_SIZE_MULTIPLICATOR) as usize].into_boxed_slice();
        for i in 0..uring_inflight_utilization.len() {
            uring_inflight_utilization[i] = self.uring_inflight_utilization[i] + other.uring_inflight_utilization[i];
        }

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
            amount_io_model_calls: self.amount_io_model_calls + other.amount_io_model_calls,
            data_rate_gbit, 
            packet_loss,
            uring_canceled_multishot: self.uring_canceled_multishot + other.uring_canceled_multishot,
            uring_cq_utilization,
            uring_inflight_utilization
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

#[derive(Debug, Serialize, Copy, Clone)]
pub struct Parameter {
    pub mode: super::NPerfMode,
    pub ip: std::net::Ipv4Addr,
    pub amount_threads: u16,
    pub output_format: OutputFormat,
    pub io_model: super::IOModel,
    pub test_runtime_length: u64,
    pub mss: u32,
    pub datagram_size: u32,
    pub packet_buffer_size: usize,
    pub socket_options: SocketOptions,
    pub exchange_function: super::ExchangeFunction,
    pub multiplex_port: MultiplexPort,
    pub multiplex_port_server: MultiplexPort,
    pub simulate_connection: SimulateConnection,
    pub core_affinity: bool,
    pub numa_affinity: bool,
    pub uring_parameter: UringParameter,
}

impl Parameter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(mode: super::NPerfMode, ip: std::net::Ipv4Addr, amount_threads: u16, output_format: OutputFormat, io_model: super::IOModel, test_runtime_length: u64, mss: u32, datagram_size: u32, packet_buffer_size: usize, socket_options: SocketOptions, exchange_function: super::ExchangeFunction, multiplex_port: MultiplexPort, multiplex_port_server: MultiplexPort, simulate_connection: SimulateConnection, core_affinity: bool, numa_affinity: bool, uring_parameter: UringParameter) -> Parameter {
        Parameter {
            mode,
            ip,
            amount_threads,
            output_format,
            io_model,
            test_runtime_length,
            mss,
            datagram_size,
            packet_buffer_size,
            socket_options,
            exchange_function,
            multiplex_port,
            multiplex_port_server,
            simulate_connection,
            core_affinity,
            numa_affinity,
            uring_parameter
        }
    }
}

#[derive(Debug, Serialize, Copy, Clone)]
pub struct UringParameter {
    pub provided_buffer: bool,
    pub multishot: bool,
    pub ring_size: u32,
    pub burst_size: u32,
    pub buffer_size: u32,
    pub sqpoll: bool,
    pub sq_filling_mode: UringSqFillingMode
}


pub mod utilization {

    use super::*;

    pub fn serialize<S>(array: &[usize], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = HashMap::new();
        for (index, &value) in array.iter().enumerate() {
            if value != 0 && value != 1 {
                map.insert(index, value);
            }
        }
        map.serialize(serializer)
    }

    #[allow(dead_code)] // Maybe needed in the future
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<usize>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map: HashMap<usize, usize> = HashMap::deserialize(deserializer)?;
        let max_index = map.keys().max().unwrap_or(&0);
        let mut array = vec![0; max_index + 1];
        for (&index, &value) in map.iter() {
            array[index] = value;
        }
        Ok(array)
    }
}
