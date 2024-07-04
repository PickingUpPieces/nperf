use std::{fs::OpenOptions, ops::{Add, Sub}, path, thread, time::{Instant, SystemTime, UNIX_EPOCH}};
use log::{debug, error, info};
use serde::Serialize;
use serde_json::{self};
use crate::{io_uring::{UringMode, UringSqFillingMode, UringTaskWork}, net::socket_options::SocketOptions};
use serde::Serializer;
use std::collections::HashMap;

#[derive(clap::ValueEnum, Default, PartialEq, Debug, Clone, Copy, Serialize)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    File
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

#[derive(Debug, Clone)]
pub struct StatisticInterval {
    interval_id: u64,
    pub output_interval: f64,
    pub last_send_instant: Instant,
    pub last_send_timestamp: f64,
    pub total_interval_outputs: u64,
    amount_interval_outputs: u64,
    pub statistics: Vec<Statistic>,
}

impl StatisticInterval {
    pub fn new(last_send_instant: Instant, output_interval: f64, runtime_length: u64) -> StatisticInterval {
        let total_interval_outputs = if output_interval == 0.0 { 0 } else { (runtime_length as f64 / output_interval).floor() as u64 };
        StatisticInterval {
            interval_id: 1,
            output_interval,
            last_send_instant,
            last_send_timestamp: Statistic::get_unix_timestamp(),
            amount_interval_outputs: 0,
            total_interval_outputs,
            statistics: Vec::with_capacity(total_interval_outputs as usize)
        }
    }

    pub fn start(&mut self, offset_in_milliseconds: Option<u64>) {
        self.last_send_instant = Instant::now() + std::time::Duration::from_millis(offset_in_milliseconds.unwrap_or(0));
        self.last_send_timestamp = Statistic::get_unix_timestamp() + offset_in_milliseconds.unwrap_or(0) as f64;
    }

    pub fn finished(&self) -> bool {
        self.amount_interval_outputs >= self.total_interval_outputs
    }

    pub fn calculate_interval(&mut self, mut statistic_new: Statistic) {
        let current_time = Instant::now();
        let current_time_unix = Statistic::get_unix_timestamp();

        if self.amount_interval_outputs >= self.total_interval_outputs {
            self.last_send_instant = current_time;
            debug!("{:?}: Last interval already sent. No more intervals to send.", thread::current().id());
            return;
        } 

        statistic_new.interval_id = self.interval_id;
        statistic_new.set_test_duration(Some(self.last_send_timestamp), Some(current_time_unix));
        statistic_new.calculate_statistics();

        // Update the last send operation instant to the current instant
        self.last_send_instant = current_time;
        self.last_send_timestamp = current_time_unix;
        self.amount_interval_outputs += 1;
        self.interval_id += 1;

        self.statistics.push(statistic_new);
    }
}


#[derive(Debug, Serialize, Clone)]
pub struct Statistic {
    #[serde(flatten)]
    pub parameter: Parameter,
    pub start_timestamp: f64,
    pub end_timestamp: f64,
    #[serde(skip_serializing)]
    pub test_duration: f64,
    pub interval_id: u64,
    pub total_data_gbyte: f64,
    pub amount_datagrams: u64,
    pub amount_data_bytes: usize,
    pub amount_reordered_datagrams: u64,
    pub amount_duplicated_datagrams: u64,
    pub amount_omitted_datagrams: i64,
    pub amount_syscalls: u64,
    pub amount_io_model_calls: u64,
    pub amount_eagain: u64,
    pub data_rate_gbit: f64,
    pub packet_loss: f64,
    pub cpu_user_time: f64,
    pub cpu_system_time: f64,
    pub cpu_total_time: f64,
    pub uring_copied_zc: u64,
    pub uring_canceled_multishot: u64,
    #[serde(with = "utilization_option_box_slice")]
    pub uring_sq_utilization: Option<Box<[usize]>>,
    #[serde(with = "utilization_option_box_slice")]
    pub uring_cq_utilization: Option<Box<[usize]>>,
    #[serde(with = "utilization_option_box_slice")]
    pub uring_inflight_utilization: Option<Box<[usize]>>,
}


// Measurement is used to measure the time of a specific statistc. Type time::Instant cannot be serialized, so it is not included in the Statistic struct.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Measurement {
    pub start_time: f64,
    pub end_time: f64,
    pub statistic: Statistic,
    pub first_packet_received: bool,
    pub last_packet_received: bool,
}

impl Statistic {
    pub fn new(parameter: Parameter) -> Statistic {
        let uring_record_utilization = parameter.uring_parameter.record_utilization;
        Statistic {
            parameter,
            start_timestamp: Self::get_unix_timestamp(),
            end_timestamp: 0.0,
            interval_id: 0,
            test_duration: 0.0,
            total_data_gbyte: 0.0,
            amount_datagrams: 0,
            amount_data_bytes: 0,
            amount_reordered_datagrams: 0,
            amount_duplicated_datagrams: 0,
            amount_omitted_datagrams: 0,
            amount_syscalls: 0,
            amount_io_model_calls: 0,
            amount_eagain: 0,
            data_rate_gbit: 0.0,
            packet_loss: 0.0,
            cpu_user_time: 0.0,
            cpu_system_time: 0.0,
            cpu_total_time: 0.0,
            uring_copied_zc: 0,
            uring_canceled_multishot: 0,
            uring_sq_utilization: if uring_record_utilization { Some(vec![0_usize; (crate::URING_MAX_RING_SIZE + 1) as usize].into_boxed_slice()) } else { None },
            uring_cq_utilization: if uring_record_utilization { Some(vec![0_usize; ((crate::URING_MAX_RING_SIZE * 2) + 1) as usize].into_boxed_slice()) } else { None },
            uring_inflight_utilization: if uring_record_utilization { Some(vec![0_usize; ((crate::URING_MAX_RING_SIZE * crate::URING_BUFFER_SIZE_MULTIPLICATOR) + 1) as usize].into_boxed_slice()) } else { None },
        }
    }

    pub fn get_unix_timestamp() -> f64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(n) => n.as_secs_f64(),
            Err(_) => panic!("Error getting the current time"),
        }
    }

    pub fn calculate_statistics(&mut self) {
        debug!("Updating statistic...");
        self.total_data_gbyte = self.calculate_total_data();
        self.data_rate_gbit = self.calculate_data_rate();
        self.packet_loss = self.calculate_packet_loss();
        debug!("Statistic updated: {:?}", self);
    }

    pub fn print(&mut self, output_format: OutputFormat, interval_print: bool) {
        self.calculate_statistics();

        match output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(&self).unwrap());
            },
            OutputFormat::Text => {
                let interval_timestamp = self.interval_id as f64 * self.parameter.output_interval;

                if interval_print {
                    println!(
                        "[{:3}] {:2.2}-{:2.2} sec  {:.2} GBytes  {:.2} Gbits/sec  {}/{} ({:.1}%)",
                        self.interval_id, 
                        if interval_timestamp == 0.0 { 0.0 } else { interval_timestamp - self.parameter.output_interval }, 
                        interval_timestamp, 
                        self.total_data_gbyte, 
                        self.data_rate_gbit, 
                        self.amount_omitted_datagrams, 
                        self.amount_datagrams, 
                        self.packet_loss
                    );
                } else {
                println!("------------------------");
                println!("Summary Measurement");
                println!("------------------------");
                println!("Total time: {:.2}s", self.test_duration);
                println!("Total data: {:.2} GiBytes", self.total_data_gbyte);
                println!("Data rate: {:.2} GiBytes/s / {:.2} Gibit/s", self.data_rate_gbit / 8.0, self.data_rate_gbit);
                println!("Packet loss: {:.2}%", self.packet_loss);
                println!("------------------------");
                println!("CPU user space: {:.2}%", self.cpu_user_time);
                println!("CPU system space: {:.2}%", self.cpu_system_time);
                println!("CPU total: {:.2}%", self.cpu_total_time);
                println!("Threads used: {}", self.parameter.amount_threads);
                println!("------------------------");
                println!("Amount of datagrams: {}", self.amount_datagrams);
                println!("Amount of reordered datagrams: {}", self.amount_reordered_datagrams);
                println!("Amount of duplicated datagrams: {}", self.amount_duplicated_datagrams);
                println!("Amount of omitted datagrams: {}", self.amount_omitted_datagrams);
                println!("Amount of syscalls: {}", self.amount_syscalls);
                println!("Amount of datagrams per syscall: {:.0}", if self.amount_syscalls != 0 { self.amount_datagrams / self.amount_syscalls } else { 0 });
                println!("Amount of IO model syscalls: {}", self.amount_io_model_calls);
                println!("Amount of received EAGAIN errors: {}", self.amount_eagain);
                println!("------------------------");
                if self.parameter.io_model == super::IOModel::IoUring {
                    println!("Io-Uring");
                    println!("------------------------");
                    println!("Copied zero-copy: {}", self.uring_copied_zc);
                    println!("Uring canceled multishot: {}", self.uring_canceled_multishot);
                    if self.parameter.uring_parameter.record_utilization {
                        println!("Uring SQ utilization:");
                        for (index, &utilization) in self.uring_sq_utilization.as_ref().unwrap().iter().enumerate() {
                            if utilization != 0 && utilization != 1 {
                                println!("SQ[{}]: {}", index, utilization);
                            }
                        }

                        println!();
                        println!("Uring CQ utilization:");
                        for (index, &utilization) in self.uring_cq_utilization.as_ref().unwrap().iter().enumerate() {
                            if utilization != 0 && utilization != 1 {
                                println!("CQ[{}]: {}", index, utilization);
                            }
                        }

                        println!();
                        println!("Uring Inflight utilization:");
                        for(index, &utilization) in self.uring_inflight_utilization.as_ref().unwrap().iter().enumerate() {
                            if utilization != 0 && utilization != 1 {
                                println!("Inflight[{}]: {}", index, utilization);
                            }
                        }
                        println!(); 
                    }
                }
            }
            },
            OutputFormat::File => {
                let mut output_file = self.parameter.output_file_path.clone();
                output_file.set_extension("csv");

                // Check if the output dir exists. If not, try to create it
                if let Some(parent_dir) = output_file.parent() {
                    if !parent_dir.exists() {
                        if let Err(err) = std::fs::create_dir_all(parent_dir) {
                            error!("Failed to create output directory: {:?}", err);
                            return;
                        } else {
                            debug!("Output directory created: {:?}", parent_dir);
                        }
                    }
                }
                let file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&output_file);

                if let Ok(file) = file {
                    // Check if the file exists is empty
                    let is_empty = file.metadata().unwrap().len() == 0;

                    // Use csv writer to write the results to a file
                    let mut wtr = if is_empty {
                        // If the file is empty, use automatically write the header and data
                        csv::Writer::from_writer(file)
                    } else {
                        // If the file is not empty, manually write the data without the header
                        csv::WriterBuilder::new().has_headers(false).from_writer(file)
                    };

                    wtr.serialize(&self).unwrap();
                    wtr.flush().unwrap();
                    info!("Results saved to {}", output_file.display());
                } else {
                    error!("Failed to create file: {}", output_file.display());
                }
            }
        }
    }

    pub fn set_start_timestamp(&mut self, start_time: Option<f64>) {
        match start_time {
            Some(time) => self.start_timestamp = time,
            None => self.start_timestamp = Self::get_unix_timestamp()
        }
    }

    pub fn set_end_timestamp(&mut self) {
        self.end_timestamp = Self::get_unix_timestamp();
    }

    fn calculate_total_data(&self) -> f64 {
        self.amount_data_bytes as f64 / 1024.0 / 1024.0 / 1024.0
    }
    
    fn calculate_data_rate(&self) -> f64{
        let elapsed_time_in_seconds = self.test_duration;
        ( self.total_data_gbyte / elapsed_time_in_seconds ) * 8.0
    }
    
    fn calculate_packet_loss(&self) -> f64 {
        (self.amount_omitted_datagrams as f64 / self.amount_datagrams as f64) * 100.0
    }
    
    pub fn set_test_duration(&mut self, start_time: Option<f64>, end_time: Option<f64>) {
        self.start_timestamp  = if let Some(time) = start_time { time } else { self.start_timestamp };
        self.end_timestamp = if let Some(time) = end_time { time } else { self.end_timestamp };
        self.test_duration = self.end_timestamp - self.start_timestamp;
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


        // Add the arrays field by field
        let (uring_inflight_utilization, uring_sq_utilization, uring_cq_utilization) = 
            if self.parameter.uring_parameter.record_utilization {
                let mut uring_sq_utilization = vec![0; (crate::URING_MAX_RING_SIZE + 1) as usize].into_boxed_slice();
                let self_uring_sq_utilization = self.uring_sq_utilization.unwrap();
                let other_uring_sq_utilization = other.uring_sq_utilization.unwrap();
                for i in 0..uring_sq_utilization.len() {
                    uring_sq_utilization[i] = self_uring_sq_utilization[i] + other_uring_sq_utilization[i];
                }

                let mut uring_cq_utilization = vec![0; ((crate::URING_MAX_RING_SIZE * 2) + 1) as usize].into_boxed_slice();
                let self_uring_cq_utilization = self.uring_cq_utilization.unwrap();
                let other_uring_cq_utilization = other.uring_cq_utilization.unwrap();
                for i in 0..uring_cq_utilization.len() {
                    uring_cq_utilization[i] = self_uring_cq_utilization[i] + other_uring_cq_utilization[i];
                }

                let mut uring_inflight_utilization = vec![0; ((crate::URING_MAX_RING_SIZE * crate::URING_BUFFER_SIZE_MULTIPLICATOR) + 1) as usize].into_boxed_slice();
                let self_uring_inflight_utilization = self.uring_inflight_utilization.unwrap();
                let other_uring_inflight_utilization = other.uring_inflight_utilization.unwrap();
                for i in 0..uring_inflight_utilization.len() {
                    uring_inflight_utilization[i] = self_uring_inflight_utilization[i] + other_uring_inflight_utilization[i];
                }
                (Some(uring_inflight_utilization), Some(uring_sq_utilization), Some(uring_cq_utilization))
            } else {
                (None, None, None)
            };

        Statistic {
            parameter: self.parameter, // Assumption is that both statistics have the same test parameters
            start_timestamp: f64::min(self.start_timestamp, other.start_timestamp),
            end_timestamp: f64::max(self.end_timestamp, other.end_timestamp),
            test_duration: f64::max(self.test_duration, other.test_duration),
            interval_id: std::cmp::max(self.interval_id, other.interval_id), // Take the bigger value
            total_data_gbyte: self.total_data_gbyte + other.total_data_gbyte,
            amount_datagrams: self.amount_datagrams + other.amount_datagrams,
            amount_data_bytes: self.amount_data_bytes + other.amount_data_bytes,
            amount_reordered_datagrams: self.amount_reordered_datagrams + other.amount_reordered_datagrams,
            amount_duplicated_datagrams: self.amount_duplicated_datagrams + other.amount_duplicated_datagrams,
            amount_omitted_datagrams: self.amount_omitted_datagrams + other.amount_omitted_datagrams,
            amount_syscalls: self.amount_syscalls + other.amount_syscalls,
            amount_io_model_calls: self.amount_io_model_calls + other.amount_io_model_calls,
            amount_eagain: self.amount_eagain + other.amount_eagain,
            data_rate_gbit, 
            packet_loss,
            cpu_user_time: 0.0,
            cpu_system_time: 0.0,
            cpu_total_time: 0.0,
            uring_copied_zc: self.uring_copied_zc + other.uring_copied_zc,
            uring_canceled_multishot: self.uring_canceled_multishot + other.uring_canceled_multishot,
            uring_sq_utilization,
            uring_cq_utilization,
            uring_inflight_utilization
        }
    }
}


impl Sub for Statistic {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        // Check if one is zero, to avoid division by zero
        let data_rate_gbit = if self.data_rate_gbit == 0.0 {
            other.data_rate_gbit
        } else if other.data_rate_gbit == 0.0 {
            self.data_rate_gbit
        } else {
            ( self.data_rate_gbit - other.data_rate_gbit ) / 2.0 // Data rate is the average of both
        };

        // Check if one is zero, to avoid division by zero
        let packet_loss = if self.packet_loss == 0.0 {
            other.packet_loss
        } else if other.packet_loss == 0.0 {
            self.packet_loss
        } else {
            ( self.packet_loss - other.packet_loss ) / 2.0 // Average of packet loss
        };

         // Add the arrays field by field
        let (uring_inflight_utilization, uring_sq_utilization, uring_cq_utilization) = 
            if self.parameter.uring_parameter.record_utilization {
                let mut uring_sq_utilization = vec![0; (crate::URING_MAX_RING_SIZE + 1) as usize].into_boxed_slice();
                let self_uring_sq_utilization = self.uring_sq_utilization.unwrap();
                let other_uring_sq_utilization = other.uring_sq_utilization.unwrap();
                for i in 0..uring_sq_utilization.len() {
                    uring_sq_utilization[i] = self_uring_sq_utilization[i] - other_uring_sq_utilization[i];
                }

                let mut uring_cq_utilization = vec![0; ((crate::URING_MAX_RING_SIZE * 2) + 1) as usize].into_boxed_slice();
                let self_uring_cq_utilization = self.uring_cq_utilization.unwrap();
                let other_uring_cq_utilization = other.uring_cq_utilization.unwrap();
                for i in 0..uring_cq_utilization.len() {
                    uring_cq_utilization[i] = self_uring_cq_utilization[i] - other_uring_cq_utilization[i];
                }

                let mut uring_inflight_utilization = vec![0; ((crate::URING_MAX_RING_SIZE * crate::URING_BUFFER_SIZE_MULTIPLICATOR) + 1) as usize].into_boxed_slice();
                let self_uring_inflight_utilization = self.uring_inflight_utilization.unwrap();
                let other_uring_inflight_utilization = other.uring_inflight_utilization.unwrap();
                for i in 0..uring_inflight_utilization.len() {
                    uring_inflight_utilization[i] = self_uring_inflight_utilization[i] - other_uring_inflight_utilization[i];
                }
                (Some(uring_inflight_utilization), Some(uring_sq_utilization), Some(uring_cq_utilization))
            } else {
                (None, None, None)
            };

        Statistic {
            parameter: self.parameter, // Assumption is that both statistics have the same test parameters
            start_timestamp: f64::min(self.start_timestamp, other.start_timestamp),
            end_timestamp: f64::max(self.end_timestamp, other.end_timestamp),
            test_duration: f64::max(self.test_duration, other.test_duration),
            interval_id: std::cmp::max(self.interval_id, other.interval_id), // Take the bigger value
            total_data_gbyte: self.total_data_gbyte - other.total_data_gbyte,
            amount_datagrams: self.amount_datagrams - other.amount_datagrams,
            amount_data_bytes: self.amount_data_bytes - other.amount_data_bytes,
            amount_reordered_datagrams: self.amount_reordered_datagrams - other.amount_reordered_datagrams,
            amount_duplicated_datagrams: self.amount_duplicated_datagrams - other.amount_duplicated_datagrams,
            amount_omitted_datagrams: self.amount_omitted_datagrams - other.amount_omitted_datagrams,
            amount_syscalls: self.amount_syscalls - other.amount_syscalls,
            amount_io_model_calls: self.amount_io_model_calls - other.amount_io_model_calls,
            amount_eagain: self.amount_eagain - other.amount_eagain,
            data_rate_gbit, 
            packet_loss,
            cpu_user_time: 0.0,
            cpu_system_time: 0.0,
            cpu_total_time: 0.0,
            uring_copied_zc: self.uring_copied_zc - other.uring_copied_zc,
            uring_canceled_multishot: self.uring_canceled_multishot - other.uring_canceled_multishot,
            uring_sq_utilization,
            uring_cq_utilization,
            uring_inflight_utilization
        }
    }
}

impl Measurement {
    pub fn new(parameter: Parameter) -> Measurement {
        Measurement {
            start_time: Statistic::get_unix_timestamp(),
            end_time: Statistic::get_unix_timestamp(),
            statistic: Statistic::new(parameter),
            first_packet_received: false,
            last_packet_received: false,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Parameter {
    pub test_name: String,
    pub run_name: String,
    pub mode: super::NPerfMode,
    pub ip: std::net::Ipv4Addr,
    pub amount_threads: u16,
    #[serde(skip_serializing)]
    pub output_interval: f64,
    #[serde(skip_serializing)]
    pub output_format: OutputFormat,
    #[serde(skip_serializing)]
    pub output_file_path: path::PathBuf,
    pub io_model: super::IOModel,
    pub test_runtime_length: u64,
    pub mss: u32,
    pub datagram_size: u32,
    pub packet_buffer_size: usize,
    #[serde(flatten)]
    pub socket_options: SocketOptions,
    pub exchange_function: super::ExchangeFunction,
    pub multiplex_port: MultiplexPort,
    pub multiplex_port_receiver: MultiplexPort,
    pub simulate_connection: SimulateConnection,
    pub core_affinity: bool,
    pub numa_affinity: bool,
    #[serde(flatten)]
    pub uring_parameter: UringParameter,
}

impl Parameter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        test_name: String,
        run_name: String,
        mode: super::NPerfMode, 
        ip: std::net::Ipv4Addr, 
        amount_threads: u16, 
        output_interval: f64,
        output_format: OutputFormat, 
        output_file_path: path::PathBuf,
        io_model: super::IOModel, 
        test_runtime_length: u64, 
        mss: u32, 
        datagram_size: u32, 
        packet_buffer_size: usize, 
        socket_options: SocketOptions, 
        exchange_function: super::ExchangeFunction, 
        multiplex_port: MultiplexPort, 
        multiplex_port_receiver: MultiplexPort, 
        simulate_connection: SimulateConnection, 
        core_affinity: bool, 
        numa_affinity: bool, 
        uring_parameter: UringParameter
    ) -> Parameter {
        Parameter {
            test_name,
            run_name,
            mode,
            ip,
            amount_threads,
            output_interval,
            output_format,
            output_file_path,
            io_model,
            test_runtime_length,
            mss,
            datagram_size,
            packet_buffer_size,
            socket_options,
            exchange_function,
            multiplex_port,
            multiplex_port_receiver,
            simulate_connection,
            core_affinity,
            numa_affinity,
            uring_parameter
        }
    }
}

#[derive(Debug, Serialize, Copy, Clone)]
pub struct UringParameter {
    pub uring_mode: UringMode,
    pub ring_size: u32,
    pub burst_size: u32,
    pub buffer_size: u32,
    pub sqpoll: bool,
    pub sqpoll_shared: bool,
    pub sq_filling_mode: UringSqFillingMode,
    pub task_work: UringTaskWork,
    pub record_utilization: bool,
}

pub mod utilization_option_box_slice {
    use super::*;
    use serde::Serializer;

    pub fn serialize<S>(value: &Option<Box<[usize]>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => {
                // Directly use the serialize function from the utilization module
                utilization::serialize(v, serializer)
            },
            None => serializer.serialize_none(),
        }
    }
}

pub mod utilization {

    use super::*;
    const LIMIT_LENGTH_OUTPUT: usize = 15;

    pub fn serialize<S>(array: &[usize], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut values = array.iter().enumerate()
            .filter(|&(_, &value)| value != 0 && value != 1)
            .collect::<Vec<_>>();
        values.sort_by(|a, b| b.1.cmp(a.1));
        values.truncate(LIMIT_LENGTH_OUTPUT);

        let mut map = HashMap::new();
        for &(index, &value) in &values {
            map.insert(index, value);
        }

        let map_string = map.iter()
            .map(|(k, v)| format!("'{}': {}", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        serializer.serialize_str(&format!("{{{}}}", map_string))
    }

}


pub mod serialize_option_as_bool {
    use serde::Serializer;

    // This function will be used to serialize Option<u32> fields
    pub fn serialize<S>(option: &Option<u32>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match option {
            Some(value) => serializer.serialize_u32(*value), 
            None => serializer.serialize_bool(false),
        }
    }
}