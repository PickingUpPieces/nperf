use std::{net::Ipv4Addr, str::FromStr};

use log::warn;

pub mod socket;
pub mod socket_options;

#[repr(u64)]
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum MessageType {
    INIT,
    MEASUREMENT,
    LAST
}

const LEN_HEADER: usize = std::mem::size_of::<MessageHeader>();

#[derive(Debug)]
#[repr(transparent)]
pub struct MessageHeader {
    header: [u64; 3]
}
// First 8 bytes: MessageType
// Second 8 bytes: Test ID
// Third 8 bytes: Packet ID

impl MessageHeader {
    pub fn new(mtype: MessageType, test_id: u64, packet_id: u64) -> MessageHeader {
        MessageHeader {
            header: [mtype as u64, test_id, packet_id]
        }
    }

    pub fn serialize(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.header.as_ptr() as *const u8, LEN_HEADER)
        }
    }

    pub fn set_packet_id(&mut self, packet_id: u64) {
        self.header[2] = packet_id;
    }

    pub fn get_packet_id(buffer: &[u8]) -> u64 {
        unsafe {
            let header = std::mem::transmute::<&[u8], &[u64]>(buffer);
            header[2]
        }
    }

    pub fn set_packet_id_raw(buffer: &mut [u8], packet_id: u64) {
        unsafe {
            let header = std::mem::transmute::<&mut [u8], &mut [u64]>(buffer);
            header[2] = packet_id;
        }
    }

    pub fn get_test_id(buffer: &[u8]) -> u64 {
        unsafe {
            let header = std::mem::transmute::<&[u8], &[u64]>(buffer);
            header[1]
        }
    }

    pub fn get_message_type(buffer: &[u8]) -> MessageType {
        unsafe {
            let header = std::mem::transmute::<&[u8], &[u64]>(buffer);
            std::mem::transmute::<u64, MessageType>(header[0])
        }
    }
        
    pub fn len(&self) -> usize {
        LEN_HEADER
    }
}


pub fn parse_ipv4(adress: &str) -> Result<Ipv4Addr, &'static str> {
    match Ipv4Addr::from_str(adress) {
        Ok(x) => Ok(x),
        Err(_) => Err("Invalid IPv4 address!"),
    }
}

#[allow(dead_code)]
pub fn parse_msg_flags(msg_flags: i32) {
    if msg_flags == 0 {
        return;
    }
    // Parse the libc msghdr msg_flags, then we need to handle the flags
    if msg_flags & libc::MSG_CTRUNC != 0 {
        warn!("Control data truncated");
    }
    if msg_flags & libc::MSG_DONTROUTE != 0 {
        warn!("Send without using routing tables");
    }
    if msg_flags & libc::MSG_EOR != 0 {
        warn!("Terminates a record (if supported by the protocol)");
    }
    if msg_flags & libc::MSG_OOB != 0 {
        warn!("Out-of-band data");
    }
    if msg_flags & libc::MSG_PEEK != 0 {
        warn!("Leave received data in queue");
    }
    if msg_flags & libc::MSG_TRUNC != 0 {
        warn!("Normal data truncated");
    }
    if msg_flags & libc::MSG_WAITALL != 0 {
        warn!("Attempt to fill the read buffer");
    }
}
