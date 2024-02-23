mod node;
mod net;
mod util;
mod command;

pub use util::statistic::Statistic;

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
const DEFAULT_AMOUNT_MSG_WHEN_SENDMMSG: usize = 1;
const DEFAULT_IO_MODEL: &str = "select";

pub use command::nPerf;