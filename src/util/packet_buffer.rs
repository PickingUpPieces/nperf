use log::debug;

use crate::net::MessageHeader;
use super::msghdr_vec::MsghdrVec;

pub struct PacketBuffer {
    pub mmsghdr_vec: Vec<libc::mmsghdr>,
    datagram_size: usize, // ASSUMPTION: It's the same for all msghdrs
    packets_amount_per_msghdr: usize, // ASSUMPTION: It's the same for all msghdrs
}

impl PacketBuffer {
    // Consumes the packet buffer vector and creates a vector of mmsghdr structs
    pub fn new(msghdr_vec: MsghdrVec) -> PacketBuffer {
        let mut mmsghdr_vec = Vec::with_capacity(msghdr_vec.msghdr_vec.len());
        let datagram_size = msghdr_vec.datagram_size();
        let packets_amount_per_msghdr = msghdr_vec.packets_amount_per_msghdr();

        for wrapper_msghdr in msghdr_vec.msghdr_vec {
            let msghdr = wrapper_msghdr.move_msghdr();
            let mmsghdr = libc::mmsghdr {
                msg_hdr: msghdr,
                msg_len: 0,
            };
            mmsghdr_vec.push(mmsghdr);
        }

        PacketBuffer {
            mmsghdr_vec,
            datagram_size,
            packets_amount_per_msghdr
        }
    }

    pub fn get_buffer_pointer_from_index(&mut self, index: usize) -> Result<&mut [u8], &'static str> {
        if let Some(mmsghdr) = self.mmsghdr_vec.get_mut(index) {
            Ok(Self::get_buffer_pointer_from_mmsghdr(mmsghdr))
        } else {
            Err("Getting buffer pointer of msghdr is out of bounds!")
        }
    }

    pub fn get_buffer_pointer_from_mmsghdr(mmsghdr: &mut libc::mmsghdr) -> &mut [u8] {
        let iov_base = unsafe { (*mmsghdr.msg_hdr.msg_iov).iov_base as *mut u8 };
        let iov_len = unsafe { (*mmsghdr.msg_hdr.msg_iov).iov_len };
        unsafe { std::slice::from_raw_parts_mut(iov_base, iov_len) }
    }

    pub fn get_msghdr_from_index(&mut self, index: usize) -> Result<&mut libc::msghdr, &'static str> {
        if let Some(mmsghdr) = self.mmsghdr_vec.get_mut(index) {
            Ok(&mut mmsghdr.msg_hdr)
        } else {
            Err("Getting msghdr is out of bounds!")
        }
    }

    pub fn add_packet_ids(&mut self, packet_id: u64) -> Result<u64, &'static str> {
        let mut amount_used_packet_ids: u64 = 0;

        // Iterate over all mmsghdr structs
        for mmsghdr in self.mmsghdr_vec.iter_mut() { 
            let msghdr_buffer = Self::get_buffer_pointer_from_mmsghdr(mmsghdr);

            for i in 0..self.packets_amount_per_msghdr {
                let start_of_packet = i * self.datagram_size;
                MessageHeader::set_packet_id_raw(&mut msghdr_buffer[start_of_packet..], packet_id + amount_used_packet_ids);
                amount_used_packet_ids += 1;
            }
        }

        debug!("Added packet IDs to buffer! Used packet IDs: {}, Next packet ID: {}", amount_used_packet_ids, packet_id + amount_used_packet_ids);
        // Return amount of used packet IDs
        Ok(amount_used_packet_ids)
    }

    pub fn packets_amount_per_msghdr(&self) -> usize {
        self.packets_amount_per_msghdr
    }

    pub fn datagram_size(&self) -> usize {
        self.datagram_size
    }
}