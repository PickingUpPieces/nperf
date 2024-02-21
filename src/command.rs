use clap::Parser;
use log::{debug, error, info};

use std::{sync::mpsc::{self, Sender}, thread};

use crate::node::{client::Client, server::Server, Node};
use crate::net::{self, socket_options::SocketOptions};
use crate::util::{self, statistic::Statistic, ExchangeFunction};

#[derive(Parser,Default,Debug)]
#[clap(version, about="A network performance measurement tool")]
#[allow(non_camel_case_types)]
pub struct nPerf {
    /// Mode of operation: client or server
    #[arg(default_value_t = String::from("server"))]
    mode: String,

    /// IP address to measure against/listen on
    #[arg(short = 'a',long, default_value_t = String::from("0.0.0.0"))]
    ip: String,

    /// Port number to measure against/listen on. If port is defined with parallel mode, all client threads will measure against the same port. 
    #[arg(short, long, default_value_t = crate::DEFAULT_PORT)]
    port: u16,

    /// Start multiple client/server threads in parallel. The port number will be incremented automatically.
    #[arg(long, default_value_t = 1)]
    parallel: u16,

    /// Don't stop the node after the first measurement
    #[arg(short, long, default_value_t = false)]
    run_infinite: bool,

    /// Set length of single datagram (Without IP and UDP headers)
    #[arg(short = 'l', long, default_value_t = crate::DEFAULT_UDP_DATAGRAM_SIZE)]
    datagram_size: u32,

    /// Time to run the test
    #[arg(short = 't', long, default_value_t = crate::DEFAULT_DURATION)]
    time: u64,

    /// Enable GSO on sending socket
    #[arg(long, default_value_t = false)]
    with_gso: bool,

    /// Set GSO buffer size which overwrites the MSS by default if GSO/GRO is enabled
    #[arg(long, default_value_t = crate::DEFAULT_GSO_BUFFER_SIZE)]
    with_gso_buffer: u32,

    /// Set transmit buffer size. Gets overwritten by GSO/GRO buffer size if GSO/GRO is enabled.
    #[arg(long, default_value_t = crate::DEFAULT_MSS)]
    with_mss: u32,

    /// Enable GRO on receiving socket
    #[arg(long, default_value_t = false)]
    with_gro: bool,

    /// Disable fragmentation on sending socket
    #[arg(long, default_value_t = false)]
    with_ip_frag: bool,

    /// Use sendmsg/recvmsg method for sending data
    #[arg(long, default_value_t = false)]
    with_msg: bool,    

    /// Use sendmmsg/recvmmsg method for sending data
    #[arg(long, default_value_t = false)]
    with_mmsg: bool, 

    /// Amount of message packs of gso_buffers to send when using sendmmsg
    #[arg(long, default_value_t = crate::DEFAULT_AMOUNT_MSG_WHEN_SENDMMSG)]
    with_mmsg_amount: usize,

    /// Enable setting udp socket buffer size
    #[arg(long, default_value_t = false)]
    with_socket_buffer: bool,

    /// Disable non-blocking socket
    #[arg(long, default_value_t = false)]
    without_non_blocking: bool,

    /// Select the IO model to use: busy-waiting, select, poll
    #[arg(long, default_value_t = crate::DEFAULT_IO_MODEL.to_string())]
    io_model: String,

    /// Enable json output of statistics
    #[arg(long, default_value_t = false)]
    json: bool,

    #[arg(long, hide = true)]
    markdown_help: bool,
}

impl nPerf {
    pub fn new() -> Self {
        let _ = env_logger::try_init();
        nPerf::parse()
    }

    pub fn exec(self) -> Option<Statistic> {
        info!("Starting nPerf...");

        let parameter = match self.parse_args() {
            Some(x) => x,
            None => { error!("Error running app"); return None; },
        };

        match Self::parameter_check(&parameter) {
            false => { error!("Invalid parameter!"); return None; },
            true => {}
        }

        debug!("Running with Parameter: {:?}", parameter);

        loop {
            let mut fetch_handle: Vec<thread::JoinHandle<()>> = Vec::new();
            let (tx, rx) = mpsc::channel();
    
            for i in 0..self.parallel {
                let tx: Sender<_> = tx.clone();
                let port = if self.port != 45001 {
                    info!("Port is set to different port than 45001. Incrementing port number is disabled.");
                    self.port
                } else {
                    self.port + i
                };
                
                fetch_handle.push(thread::spawn(move || {
                    let mut node:Box<dyn Node> = if parameter.mode == util::NPerfMode::Client {
                        Box::new(Client::new(i as u64, parameter.ip, port, parameter))
                    } else {
                        Box::new(Server::new(parameter.ip, port, parameter))
                    };
    
                    match node.run(parameter.io_model) {
                        Ok(statistic) => { 
                            info!("Finished measurement!");
                            tx.send(Some(statistic)).unwrap();
                        },
                        Err(x) => {
                            error!("Error running app: {}", x);
                            tx.send(None).unwrap();
                        }
                    }
                }));
            }
    
            info!("Waiting for all threads to finish...");
            let mut statistic = fetch_handle.into_iter().fold(Statistic::new(parameter), |acc: Statistic, handle| { 
                let stat = acc + match rx.recv_timeout(std::time::Duration::from_secs(parameter.test_runtime_length * 2)).expect("Timeout") {
                    Some(x) => x,
                    None => Statistic::new(parameter)
                };
                handle.join().unwrap(); 
                stat 
            });
            info!("All threads finished!");
    
            if statistic.amount_datagrams != 0 {
                statistic.calculate_statistics();
                statistic.print(parameter.output_format);
            }
            if !(self.run_infinite && parameter.mode == util::NPerfMode::Server) {
                return Some(statistic);
            }
        }
    }

    pub fn set_args(self, args: Vec<&str>) -> Self {
        let mut args = args;
        args.insert(0, "nPerf");
        let args: Vec<String> = args.iter().map(|x| x.to_string()).collect();
        nPerf::parse_from(args)
    }

    fn parse_args(&self) -> Option<util::statistic::Parameter> {
        if self.markdown_help {
            clap_markdown::print_help_markdown::<nPerf>();
            return None;
        }
    
        let mode: util::NPerfMode = match util::parse_mode(&self.mode) {
            Some(x) => x,
            None => { error!("Invalid mode! Should be 'client' or 'server'"); return None; },
        };
    
        let ipv4 = match net::parse_ipv4(&self.ip) {
            Ok(x) => x,
            Err(_) => { error!("Invalid IPv4 address!"); return None; },
        };
    
        let (exchange_function, packet_buffer_size) = if self.with_msg {
            (ExchangeFunction::Msg, 1)
        } else if self.with_mmsg {
            (ExchangeFunction::Mmsg, self.with_mmsg_amount)
        } else {
            (ExchangeFunction::Normal, 1)
        };
        info!("Exchange function used: {:?}", exchange_function);
        
        let mss = if self.with_gso || self.with_gro {
            info!("GSO/GRO enabled with buffer size {}", self.with_gso_buffer);
            self.with_gso_buffer
        } else {
            self.with_mss
        };
        info!("MSS used: {}", mss);
    
        let io_model = match self.io_model.as_str() {
            "busy-waiting" => util::IOModel::BusyWaiting,
            "select" => util::IOModel::Select,
            "poll" =>util::IOModel::Poll,
            _ => { error!("Invalid IO model! Should be 'busy-waiting', 'select' or 'poll'"); return None; },
        };
        info!("IO model used: {:?}", io_model);
        info!("Output format: {}", if self.json {"json"} else {"text"});
        info!("UDP datagram size used: {}", self.datagram_size);

        let socket_options = self.parse_socket_options();

        Some(util::statistic::Parameter::new(
            mode, 
            ipv4, 
            self.parallel,
            if self.port == 45001 { self.parallel } else { 1 },
            if self.json {util::statistic::OutputFormat::Json} else {util::statistic::OutputFormat::Text}, 
            io_model, 
            self.time, 
            mss, 
            self.datagram_size, 
            packet_buffer_size, 
            socket_options, 
            exchange_function
        ))
    }

    fn parameter_check(parameter: &util::statistic::Parameter)-> bool {
        if parameter.datagram_size > crate::MAX_UDP_DATAGRAM_SIZE {
            error!("UDP datagram size is too big! Maximum is {}", crate::MAX_UDP_DATAGRAM_SIZE);
            return false;
        }

        if parameter.mode == util::NPerfMode::Client && parameter.socket_options.gro {
            error!("GRO is not supported on sending socket!");
            return false;
        }
        if parameter.mode == util::NPerfMode::Server && parameter.socket_options.gso.is_some() {
            error!("GSO is not supported on receiving socket!");
            return false;
        }
        true
    }

    fn parse_socket_options(&self) -> SocketOptions {
        let gso = if self.with_gso {
            Some(self.datagram_size)
        } else {
            None
        };

        let (recv_buffer_size, send_buffer_size) = if self.with_socket_buffer {
            info!("Setting udp buffer sizes with recv {} and send {}", crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE, crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE);
            (Some(crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE), Some(crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE))
        } else {
            info!("Setting buffer size of UDP socket disabled!");
            (None, None)
        };
        
        SocketOptions::new(
            !self.without_non_blocking, 
            self.with_ip_frag, 
            gso, 
            self.with_gro, 
            recv_buffer_size, 
            send_buffer_size
        )
    }
}