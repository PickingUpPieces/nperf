use std::{sync::mpsc::{self, Sender}, thread};
use clap::Parser;
use log::{info, error, debug};

use crate::node::{client::Client, server::Server, Node};
use crate::net::socket_options::SocketOptions;
use crate::util::{statistic::Statistic, ExchangeFunction};

mod node;
mod net;
mod util;

// const UDP_RATE: usize = (1024 * 1024) // /* 1 Mbps */
const DEFAULT_MSS: u32= 1472;
const DEFAULT_UDP_DATAGRAM_SIZE: u32 = 1472;
const DEFAULT_GSO_BUFFER_SIZE: u32= 65507;
const DEFAULT_SOCKET_SEND_BUFFER_SIZE: u32 = 26214400; // 25MB; // The buffer size will be doubled by the kernel to account for overhead. See man 7 socket
const DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE: u32 = 26214400; // 25MB; // The buffer size will be doubled by the kernel to account for overhead. See man 7 socket
const DEFAULT_DURATION: u64 = 10; // /* seconds */
const DEFAULT_PORT: u16 = 45001;
const WAIT_CONTROL_MESSAGE: u64 = 200; // /* milliseconds */

// /* Maximum datagram size UDP is (64K - 1) - IP and UDP header sizes */
const MAX_UDP_DATAGRAM_SIZE: u32 = 65535 - 8 - 20;

const DEFAULT_AMOUNT_MSG_WHEN_SENDMMSG: usize = 1024;

const DEFAULT_IO_MODEL: &str = "select";

#[derive(Parser,Default,Debug)]
#[clap(version, about="A network performance measurement tool")]
struct Arguments{
    /// Mode of operation: client or server
    #[arg(default_value_t = String::from("server"))]
    mode: String,

    /// IP address to measure against/listen on
    #[arg(short = 'a',long, default_value_t = String::from("0.0.0.0"))]
    ip: String,

    /// Port number to measure against/listen on. If port is defined with parallel mode, all client threads will measure against the same port. 
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Start multiple client/server threads in parallel. The port number will be incremented automatically.
    #[arg(long, default_value_t = 1)]
    parallel: u16,

    /// Don't stop the node after the first measurement
    #[arg(short, long, default_value_t = false)]
    run_infinite: bool,

    /// Set length of single datagram (Without IP and UDP headers)
    #[arg(short = 'l', long, default_value_t = DEFAULT_UDP_DATAGRAM_SIZE)]
    datagram_size: u32,

    /// Time to run the test
    #[arg(short = 't', long, default_value_t = DEFAULT_DURATION)]
    time: u64,

    /// Enable GSO on sending socket
    #[arg(long, default_value_t = false)]
    with_gso: bool,

    /// Set GSO buffer size which overwrites the MSS by default if GSO/GRO is enabled
    #[arg(long, default_value_t = DEFAULT_GSO_BUFFER_SIZE)]
    with_gso_buffer: u32,

    /// Set transmit buffer size. Gets overwritten by GSO/GRO buffer size if GSO/GRO is enabled.
    #[arg(long, default_value_t = DEFAULT_MSS)]
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
    #[arg(long, default_value_t = DEFAULT_AMOUNT_MSG_WHEN_SENDMMSG)]
    with_mmsg_amount: usize,

    /// Enable non-blocking socket
    #[arg(long, default_value_t = false)]
    without_non_blocking: bool,

    /// Select the IO model to use: busy-waiting, select, poll
    #[arg(long, default_value_t = DEFAULT_IO_MODEL.to_string())]
    io_model: String,

    /// Enable json output of statistics
    #[arg(long, default_value_t = false)]
    json: bool,

    #[arg(long, hide = true)]
    markdown_help: bool,
}

fn main() {
    env_logger::init();
    let args = Arguments::parse();
    debug!("{:?}", args);

    if args.markdown_help {
        clap_markdown::print_help_markdown::<Arguments>();
        return;
    }

    let mode: util::NPerfMode = match util::parse_mode(args.mode) {
        Some(x) => x,
        None => { error!("Invalid mode! Should be 'client' or 'server'"); panic!()},
    };

    let ipv4 = match net::parse_ipv4(args.ip) {
        Ok(x) => x,
        Err(_) => { error!("Invalid IPv4 address!"); panic!()},
    };

    let (exchange_function, packet_buffer_size) = if args.with_msg {
        (ExchangeFunction::Msg, 1)
    } else if args.with_mmsg {
        (ExchangeFunction::Mmsg, args.with_mmsg_amount)
    } else {
        (ExchangeFunction::Normal, 1)
    };
    info!("Exchange function used: {:?}", exchange_function);
    
    let mss = if args.with_gso || args.with_gro {
        info!("GSO/GRO enabled with buffer size {}", args.with_gso_buffer);
        args.with_gso_buffer
    } else {
        args.with_mss
    };
    info!("MSS used: {}", mss);

    let io_model = match args.io_model.as_str() {
        "busy-waiting" => util::IOModel::BusyWaiting,
        "select" => util::IOModel::Select,
        "poll" =>util::IOModel::Poll,
        _ => { error!("Invalid IO model! Should be 'busy-waiting', 'select' or 'poll'"); panic!()},
    };
    info!("IO model used: {:?}", io_model);
    info!("Output format: {}", if args.json {"json"} else {"text"});
    info!("UDP datagram size used: {}", args.datagram_size);

    
    let socket_options = SocketOptions::new(
        !args.without_non_blocking, 
        args.with_ip_frag, 
        (args.with_gso, args.datagram_size), 
        args.with_gro, 
        crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE, 
        crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE
    );

    let parameter = util::statistic::Parameter {
        mode,
        enable_json_output: args.json,
        io_model,
        test_runtime_length: args.time,
        mss,
        datagram_size: args.datagram_size,
        packet_buffer_size,
        socket_options,
        exchange_function
    };

    match parameter_check(&parameter, &socket_options) {
        false => { error!("Invalid parameter! Exiting..."); return; },
        true => {}
    }

    loop {
        let mut fetch_handle: Vec<thread::JoinHandle<()>> = Vec::new();
        let (tx, rx) = mpsc::channel();

        for i in 0..args.parallel {
            let tx: Sender<_> = tx.clone();
            
            fetch_handle.push(thread::spawn(move || {
                let port = if args.port != 45001 {
                    info!("Port is set to different port than 45001. Incrementing port number is disabled.");
                    args.port
                } else {
                    args.port + i
                };

                let mut node:Box<dyn Node> = if mode == util::NPerfMode::Client {
                    Box::new(Client::new(i, ipv4, port, parameter))
                } else {
                    Box::new(Server::new(ipv4, port, parameter))
                };

                match node.run(io_model) {
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
            let stat = acc + match rx.recv().unwrap() {
                Some(x) => x,
                None => Statistic::new(parameter)
            };
            handle.join().unwrap(); 
            stat 
        });
        info!("All threads finished!");

        if statistic.amount_datagrams != 0 {
            statistic.calculate_statistics();
            statistic.print();
        }
        if !(args.run_infinite && mode == util::NPerfMode::Server) {
            return;
        }
    }
}

fn parameter_check(parameter: &util::statistic::Parameter, socket_options: &SocketOptions)-> bool {
    if parameter.datagram_size > MAX_UDP_DATAGRAM_SIZE {
        error!("UDP datagram size is too big! Maximum is {}", MAX_UDP_DATAGRAM_SIZE);
        return false;
    }

    if parameter.mode == util::NPerfMode::Client && socket_options.gro {
        error!("GRO is not supported on sending socket!");
        return false;
    }
    if parameter.mode == util::NPerfMode::Server && socket_options.gso.0 {
        error!("GSO is not supported on receiving socket!");
        return false;
    }
    true
}
