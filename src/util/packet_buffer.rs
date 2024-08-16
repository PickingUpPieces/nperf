use log::debug;

use crate::net::MessageHeader;
use super::msghdr_vec::MsghdrVec;

pub struct PacketBuffer {
    pub mmsghdr_vec: Vec<libc::mmsghdr>,
    datagram_size: usize, // ASSUMPTION: It's the same for all msghdrs
    packets_amount_per_msghdr: usize, // ASSUMPTION: It's the same for all msghdrs
    index_pool: Vec<usize> // When buffers are used for io_uring, we need to know which buffers can be reused. VecDeque (RingBuffer) would be more logical, but is less performant.
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
            index_pool: (0..mmsghdr_vec.len()).collect(),
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

    #[allow(dead_code)]
    pub fn reset_msghdr_fields(&mut self) {
        // Reset msg_flags to 0 and msg_controllen to LENGTH_CONTROL_MESSAGE_BUFFER. 
        self.mmsghdr_vec.iter_mut().for_each(|mmsghdr| {
            mmsghdr.msg_hdr.msg_flags = 0;
            mmsghdr.msg_hdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
        });
    }

    pub fn add_packet_ids(&mut self, packet_id: u64, amount_packets: Option<usize>) -> Result<u64, &'static str> {
        let mut amount_used_packet_ids: u64 = 0;
        let mmsghdr_vec_len = self.mmsghdr_vec.len();

        // Iterate over all mmsghdr structs (or up to amount_packets if specified)
        for mmsghdr in self.mmsghdr_vec.iter_mut().take(amount_packets.unwrap_or(mmsghdr_vec_len)) {
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

    pub fn add_packet_ids_to_msghdr(&mut self, packet_id: u64, index: usize) -> Result<u64, &'static str> {
        let mut amount_used_packet_ids: u64 = 0;
        let datagram_size = self.datagram_size;
        let packets_amount_per_msghdr = self.packets_amount_per_msghdr;
        let msghdr_buffer = self.get_buffer_pointer_from_index(index)?;

        for i in 0..packets_amount_per_msghdr {
            let start_of_packet = i * datagram_size;
            MessageHeader::set_packet_id_raw(&mut msghdr_buffer[start_of_packet..], packet_id + amount_used_packet_ids);
            amount_used_packet_ids += 1;
        }

        debug!("Added packet IDs to buffer! Used packet IDs: {}, Next packet ID: {}", amount_used_packet_ids, packet_id + amount_used_packet_ids);
        Ok(amount_used_packet_ids)
    }

    pub fn packets_amount_per_msghdr(&self) -> usize {
        self.packets_amount_per_msghdr
    }

    pub fn datagram_size(&self) -> usize {
        self.datagram_size
    }

    pub fn get_buffer_index(&mut self) -> Result<usize, &'static str> {
        match self.index_pool.pop() {
            Some(index) => Ok(index),
            None => Err("No buffers left in packet_buffer")
        }
    }

    pub fn get_pool_inflight(&mut self) -> usize {
        self.index_pool.capacity() - self.index_pool.len()
    }

    pub fn return_buffer_index(&mut self, mut buf_index_vec: Vec<usize>) {
        self.index_pool.append(&mut buf_index_vec)
    }
}