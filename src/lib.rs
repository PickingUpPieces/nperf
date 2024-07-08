mod node;
mod net;
mod util;
mod command;
mod executor;
mod io_uring;

use std::net::Ipv4Addr;

pub use util::statistic::Statistic;

// const UDP_RATE: usize = (1024 * 1024) // /* 1 Mbps */
const DEFAULT_MSS: u32= 1472;
const DEFAULT_UDP_DATAGRAM_SIZE: u32 = 1472;
const DEFAULT_GSO_BUFFER_SIZE: u32 = 64768 ; // 65507 is the possible maximum , but then the last packet is only have full -> 1472 * 44 = 64768
//const MAX_SOCKET_BUFFER_SIZE: u32 = 2129920; // 2MB; // The buffer size will be doubled by the kernel to account for overhead. See man 7 socket
const DEFAULT_SOCKET_BUFFER_SIZE: u32 = 212992; 
const DEFAULT_DURATION: u64 = 10; // /* seconds */
const DEFAULT_RECEIVER_PORT: u16 = 45001;
const DEFAULT_SENDER_IP: Ipv4Addr = Ipv4Addr::new(0,0,0,0);
const DEFAULT_SENDER_PORT: u16 = 46001;
const DEFAULT_INTERVAL: f64 = 0.0; // /* seconds */
const DEFAULT_BANDWIDTH: u64 = 0; // /* Megabits per second (Mbit/s) */
const WAIT_CONTROL_MESSAGE: u64 = 400; // /* milliseconds */
const DEFAULT_FILE_NAME: &str = "nperf-output.csv";
const MAX_TEST_ID: usize = 1024;

// Maximum datagram size UDP is (64K - 1) - IP and UDP header sizes 
const MAX_UDP_DATAGRAM_SIZE: u32 = 65535 - 8 - 20;
const DEFAULT_AMOUNT_MSG_WHEN_SENDMMSG: usize = 1;
const LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER: usize = 24;

// uring defaults
const DEFAULT_URING_RING_SIZE: u32 = 256;
const URING_BUFFER_SIZE_MULTIPLICATOR: u32 = 4; // The buffer size is a multiplicator of the ring size
const URING_BURST_SIZE_DIVIDEND: u32 = 4; // The burst size is a division of the ring size
const URING_MAX_RING_SIZE: u32 = 2048; // Maximum ring size
const URING_BUFFER_GROUP: u16 = 0;
const URING_ADDITIONAL_BUFFER_LENGTH: i32 = 40;
const URING_ENTER_TIMEOUT: u32 = 10_000_000;
const URING_SQPOLL_CPU: u32 = 0;

pub use command::nPerf;