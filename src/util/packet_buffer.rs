use std::io::IoSlice;

use log::{debug, trace, info, warn};
use super::history::History;

#[derive(Debug)]
pub struct PacketBuffer {
    buffer: Vec<u8>,
    datagram_size: u32,
    _last_packet_size: u32,
    packets_amount: usize,
}

impl PacketBuffer {
    pub fn new(mss: u32, datagram_size: u32) -> Option<Self> {
        let buffer = vec![0; mss.try_into().unwrap()];

        let rest_of_buffer = mss % datagram_size;
        let _last_packet_size = if rest_of_buffer == 0 {
            info!("Buffer length is a multiple of packet size!");
            datagram_size
        } else {
            warn!("Buffer length is not a multiple of packet size! Last packet size is: {}", rest_of_buffer);
            rest_of_buffer
        };

        let packets_amount = (mss as f64 / datagram_size as f64).ceil() as usize;
        debug!("Created PacketBuffer with datagram size: {}, last packet size: {}, buffer length: {}, packets amount: {}", datagram_size, _last_packet_size, mss, packets_amount);

        Some(PacketBuffer {
            buffer,
            datagram_size,
            _last_packet_size,
            packets_amount,
        })
    }

    pub fn create_msghdr(&mut self) -> libc::msghdr {
        let mut msghdr: libc::msghdr = unsafe { std::mem::zeroed() };
    
        let iov = Box::new(libc::iovec {
            iov_base: self.buffer.as_mut_ptr() as *mut _,
            iov_len: self.buffer.len(),
        });
    
        msghdr.msg_name = std::ptr::null_mut();
        msghdr.msg_namelen = 0;
        msghdr.msg_iov = Box::into_raw(iov) as *mut _ as _;
        msghdr.msg_iovlen = 1;
        msghdr.msg_control = std::ptr::null_mut();
        msghdr.msg_controllen = 0;
    
        msghdr
    }

    pub fn get_buffer_pointer(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    pub fn get_buffer_length(&self) -> usize {
        self.buffer.len()
    }

    // Similar to iperf3's fill_with_repeating_pattern
    pub fn fill_with_repeating_pattern(&mut self) {
        let mut counter: u8 = 0;
        for i in self.get_buffer_pointer().iter_mut() {
            *i = (48 + counter).to_ascii_lowercase();

            if counter > 9 {
                counter = 0;
            } else {
                counter += 1;
            }
        }

        debug!("Filled buffer of size {} with repeating pattern", self.buffer.len());
        trace!("Filled buffer with {:?}", self.buffer);
    }

    // Iterate over all packets and add the packet ID starting from next_packet_id
    pub fn add_packet_ids(&mut self, next_packet_id: u64) -> Result<u64, &'static str> {
        let mut amount_used_packet_ids: u64 = 0;

        for i in 0..self.packets_amount {
            let start_of_packet = i * self.datagram_size as usize;
            let buffer = &mut self.buffer[start_of_packet..(start_of_packet+8)];
            buffer[0..8].copy_from_slice(&(next_packet_id + amount_used_packet_ids as u64).to_be_bytes());
            debug!("Prepared packet number: {}", u64::from_be_bytes(buffer[0..8].try_into().unwrap()));
            amount_used_packet_ids += 1;
        }

        debug!("Added packet IDs to buffer! Used packet IDs: {}, Next packet ID: {}", amount_used_packet_ids, next_packet_id + amount_used_packet_ids as u64);
        // Return amount of used packet IDs
        Ok(amount_used_packet_ids)
    }

    pub fn process_packet_msghdr(&self, msghdr: &mut libc::msghdr, amount_received_bytes: usize, next_packet_id: u64, history: &mut History) -> (u64, u64) {
        let mut absolut_packets_received = 0;
        let mut next_packet_id = next_packet_id;
        let single_packet_size = match super::get_gso_size_from_cmsg(msghdr) {
            Some(gso_size) => gso_size,
            None => {
                debug!("No GSO size received in cmsg. Assuming that only one packet was received with size ");
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

        assert_eq!(iovec.iov_len, amount_received_bytes as _);

        let datagrams: IoSlice = unsafe {
            IoSlice::new(std::slice::from_raw_parts(iovec.iov_base as *const u8, iovec.iov_len))
        };

        for packet in datagrams.chunks(single_packet_size as usize) {
            next_packet_id += super::process_packet(packet, next_packet_id, history);
            absolut_packets_received += 1;
            trace!("iovec buffer: {:?} with now absolut packets received {} and next packet id: {}", packet, next_packet_id, absolut_packets_received);
        }
    
        (next_packet_id, absolut_packets_received)
    } 
}