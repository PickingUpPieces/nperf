pub mod statistic;
pub mod packet_buffer;

use std::io::IoSlice;

use libc::mmsghdr;
use log::{debug, trace};
use serde::Serialize;
use statistic::Statistic;

use self::packet_buffer::PacketBuffer;

#[derive(PartialEq, Debug, Copy, Clone, Serialize)]
pub enum NPerfMode {
    Client,
    Server,
}

#[derive(PartialEq, Debug, Copy, Clone, Serialize)]
pub enum ExchangeFunction {
    Normal,
    Msg,
    Mmsg
}

#[derive(PartialEq, Debug, Copy, Clone, Serialize)]
pub enum IOModel {
    BusyWaiting,
    Select,
    Poll,
}

pub fn parse_mode(mode: String) -> Option<NPerfMode> {
    match mode.as_str() {
        "client" => Some(NPerfMode::Client),
        "server" => Some(NPerfMode::Server),
        _ => None,
    }
}

pub fn process_packet(buffer: &[u8], next_packet_id: u64, statistic: &mut Statistic) -> u64 {
    let packet_id = u64::from_be_bytes(buffer[0..8].try_into().unwrap());
    debug!("Received packet number: {}", packet_id);
    process_packet_number(packet_id, next_packet_id, statistic)
}

// Packet reordering taken from iperf3 and rperf https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/stream/udp.rs#L225
// https://github.com/opensource-3d-p/rperf/blob/14d382683715594b7dce5ca0b3af67181098698f/src/stream/udp.rs#L225 
fn process_packet_number(packet_id: u64, next_packet_id: u64, statistic: &mut Statistic) -> u64 {
    if packet_id == next_packet_id {
        return 1
    } else if packet_id > next_packet_id {
        let lost_packet_count = packet_id - next_packet_id;
        statistic.amount_omitted_datagrams += lost_packet_count as i64;
        debug!("Reordered or lost packet received! Expected number {}, but received {}. {} packets are currently missing", next_packet_id, packet_id, lost_packet_count);
        return lost_packet_count + 1; // This is the next packet id that we expect, since we assume that the missing packets are lost
    } else { // If the received packet_id is smaller than the expected, it means that we received a reordered (or duplicated) packet.
        if statistic.amount_omitted_datagrams > 0 { 
            statistic.amount_omitted_datagrams -= 1;
            statistic.amount_reordered_datagrams  += 1;
            debug!("Received reordered packet number {}, but expected {}", packet_id, next_packet_id);
        } else { 
            statistic.amount_duplicated_datagrams += 1;
            debug!("Received duplicated packet");
        }
        return 0
    }
}

pub fn get_gso_size_from_cmsg(msghdr: &mut libc::msghdr) -> Option<u32> {
    let mut cmsg: *mut libc::cmsghdr = unsafe { libc::CMSG_FIRSTHDR(msghdr) };
    while !cmsg.is_null() {
        let level = unsafe { (*cmsg).cmsg_level };
        let cmsg_type = unsafe { (*cmsg).cmsg_type };
        let cmsg_len = unsafe { (*cmsg).cmsg_len };
        debug!("Received cmsg with level: {}, type: {}, len: {}", level, cmsg_type, cmsg_len);

        if level == libc::SOL_UDP && cmsg_type == libc::UDP_GRO {
            let data_ptr = unsafe { libc::CMSG_DATA(cmsg) };
            let gso_size = unsafe { *(data_ptr as *const u32) };
            debug!("Received GSO size in cmsg: {}", gso_size);
            return Some(gso_size);
        }

        cmsg = unsafe { libc::CMSG_NXTHDR(msghdr, cmsg) };
    }
    None
}

pub fn process_packet_msghdr(msghdr: &mut libc::msghdr, amount_received_bytes: usize, next_packet_id: u64, statistic: &mut Statistic) -> (u64, u64) {
    let mut absolut_packets_received = 0;
    let mut next_packet_id = next_packet_id;
    let single_packet_size = match get_gso_size_from_cmsg(msghdr) {
        Some(gso_size) => gso_size,
        None => {
            debug!("No GSO size received in cmsg. Assuming that only one packet was received with size {}", amount_received_bytes);
            amount_received_bytes as u32
        }
    };

    debug!("Process packet msghdr to extract the packets received. Received {} iov packets. Start iterating over them...", msghdr.msg_iovlen);
    // Make sure that iovlen == 1, since we only support one packet per msghdr.
    let iovec = if msghdr.msg_iovlen == 1 {
        unsafe { *msghdr.msg_iov }
    } else {
        panic!("Received more than one packet in one msghdr. This is not supported yet!"); 
    };

    let datagrams: IoSlice = unsafe {
        IoSlice::new(std::slice::from_raw_parts(iovec.iov_base as *const u8, amount_received_bytes))
    };

    for packet in datagrams.chunks(single_packet_size as usize) {
        next_packet_id += process_packet(packet, next_packet_id, statistic);
        absolut_packets_received += 1;
        trace!("iovec buffer: {:?} with now absolut packets received {} and next packet id: {}", packet, next_packet_id, absolut_packets_received);
    }

    (next_packet_id, absolut_packets_received)
} 

pub fn create_mmsghdr_vec(packet_buffer_vec: &mut [PacketBuffer], with_cmsg: bool) -> Vec<libc::mmsghdr> {
    let mut mmsghdr_vec: Vec<libc::mmsghdr> = Vec::new();
    for packet_buffer in packet_buffer_vec.iter_mut() {
        let mut msghdr = packet_buffer.create_msghdr();

        if with_cmsg {
            packet_buffer.add_cmsg_buffer(&mut msghdr);
        }

        let mmsghdr = create_mmsghdr(msghdr);
        mmsghdr_vec.push(mmsghdr);
    }
    mmsghdr_vec
}

fn create_mmsghdr(msghdr: libc::msghdr) -> libc::mmsghdr {
    mmsghdr { 
        msg_hdr: msghdr, 
        msg_len: 0 // Is set to transmitted bytes by sendmmsg 
    }
}

pub fn get_total_bytes(mmsghdr_vec: &[libc::mmsghdr], amount_msghdr: usize) -> usize {
    let mut amount_sent_bytes = 0;
    for (index, mmsghdr) in mmsghdr_vec.iter().enumerate() {
        if index >= amount_msghdr {
            break;
        }
        amount_sent_bytes += mmsghdr.msg_len;
    }
    debug!("Total amount of sent bytes: {}", amount_sent_bytes);
    amount_sent_bytes as usize
}