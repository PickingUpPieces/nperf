use std::{net::Ipv4Addr, str::FromStr};
use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};

pub mod socket;
pub mod socket_options;

#[repr(u8)]
#[derive(Serialize, Deserialize, Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum MessageType {
    INIT = 0,
    MEASUREMENT = 1,
    LAST = 2
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageHeader {
    pub mtype: MessageType,
    pub test_id: u16,
    pub packet_id: u64
}

impl MessageHeader {
    pub fn new(mtype: MessageType, test_id: u16, packet_id: u64) -> MessageHeader {
        MessageHeader {
            mtype,
            test_id,
            packet_id
        }
    }
    pub fn serialize(&self) -> Vec<u8> {
        serialize(&self).unwrap()
    }

    pub fn deserialize(buffer: &[u8]) -> MessageHeader {
        // TODO: Currently static serde buffer size
        deserialize::<MessageHeader>(&buffer[0..14]).unwrap()
    }
}

pub fn parse_ipv4(adress: &str) -> Result<Ipv4Addr, &'static str> {
    match Ipv4Addr::from_str(adress) {
        Ok(x) => Ok(x),
        Err(_) => Err("Invalid IPv4 address!"),
    }
}
