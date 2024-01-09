
use clap::Parser;
mod util;
mod net;

#[derive(Parser,Default,Debug)]
#[clap(author="Vincent Picking", version, about="A network performance measurement tool")]
struct Arguments{
    // Mode of operation: client or server
    #[arg(default_value_t = String::from("server"))]
    mode: String,
    // IP address to measure against/listen on
    #[arg(default_value_t = String::from("0.0.0.0"))]
    ip: String,
    // Port number to measure against/listen on 
    #[arg(default_value_t = 45001)]
    port: u16,
}

fn main() {
    let args = Arguments::parse();
    println!("{:?}", args);

    let mode: util::NPerfMode = match util::parse_mode(args.mode) {
        Some(x) => x,
        None => panic!("Invalid mode! Should be 'client' or 'server'"),
    };

    let ipv4 = match net::parse_ipv4(args.ip) {
        Ok(x) => x,
        Err(_) => panic!("Invalid IPv4 address!"),
    };

    let mut new_measurement = util::NperfMeasurement {
        mode,
        ip: ipv4,
        local_port: args.port,
        remote_port: 0,
        socket: 0,
        data_rate: 0,
        packet_count: 0,
        omitted_packet_count: 0,
    };

    new_measurement.socket = match net::create_socket() {
        Ok(x) => x,
        Err(x) => panic!("{x}"),
    };

    if new_measurement.mode == util::NPerfMode::Client {
        start_client(new_measurement);
    } else {
        start_server(new_measurement);
    }
    
}

fn start_server(new_measurement: util::NperfMeasurement) {
    println!("Server mode");
    match net::bind_socket(new_measurement.socket, new_measurement.ip, new_measurement.local_port) {
        Ok(_) => println!("Bound socket to port"),
        Err(x) => panic!("{x}"),
    };
}

fn start_client(new_measurement: util::NperfMeasurement) {
    println!("Client mode");
    match net::connect(new_measurement.socket, new_measurement.ip, new_measurement.local_port) {
        Ok(_) => println!("Connected to remote host"),
        Err(x) => panic!("{x}"),
    };
}
