use std::net::Ipv4Addr;

use log::debug;

#[derive(PartialEq)]
pub enum NPerfMode {
    Client,
    Server,
}

pub struct NperfMeasurement<'a> {
    pub mode: NPerfMode,
    pub ip: Ipv4Addr,
    pub local_port: u16,
    pub remote_port: u16,
    pub buffer: &'a mut [u8; crate::DEFAULT_UDP_BLKSIZE],
    pub socket: i32,
    pub data_rate: u64,
    pub packet_count: u64,
    pub omitted_packet_count: u64,
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
// TODO: Fill buffer with data
/*
 * Fills buffer with repeating pattern (similar to pattern that used in iperf2)
 */
// void fill_with_repeating_pattern(void *out, size_t outsize)
// {
//     size_t i;
//     int counter = 0;
//     char *buf = (char *)out;
// 
//     if (!outsize) return;
// 
//     for (i = 0; i < outsize; i++) {
//         buf[i] = (char)('0' + counter);
//         if (counter >= 9)
//             counter = 0;
//         else
//             counter++;
//     }