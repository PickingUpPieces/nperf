use std::mem::MaybeUninit;

use log::debug;
use crate::net::{MessageHeader, MessageType};

#[allow(non_camel_case_types)]
pub struct WrapperMsghdr {
    msghdr: libc::msghdr,
    buffer_length: usize,
    with_cmsg: bool,
    sockaddr: libc::sockaddr_in,
    pub datagram_size: u32,
    pub packets_amount: usize,
}

impl WrapperMsghdr {
    pub fn new(mss: u32, datagram_size: u32) -> Option<Self> {

        let rest_of_buffer = mss % datagram_size;
        let _last_packet_size = if rest_of_buffer == 0 {
            debug!("Buffer length is a multiple of packet size!");
            datagram_size
        } else {
            debug!("Buffer length is not a multiple of packet size! Last packet size is: {}", rest_of_buffer);
            rest_of_buffer
        };

        let packets_amount = (mss as f64 / datagram_size as f64).ceil() as usize;
        debug!("Created msghdr with datagram size: {}, last packet size: {}, buffer length: {}, packets amount: {}", datagram_size, _last_packet_size, mss, packets_amount);

        let buffer = Box::leak(vec![0_u8; mss as usize].into_boxed_slice()); // Could solve using the heap by using always a MAX_PACKET_SIZE buffer (which is 2^16)
        let iov = Self::create_iovec(buffer);

        let msghdr = Self::create_msghdr(iov);
        let sockaddr: libc::sockaddr_in = unsafe { MaybeUninit::zeroed().assume_init() };

        Some(WrapperMsghdr {
            msghdr,
            buffer_length: mss as usize,
            with_cmsg: false,
            sockaddr,
            datagram_size,
            packets_amount,
        })
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
    }

    pub fn add_message_header(&mut self, test_id: u64, packet_id: u64) -> Result<u64, &'static str> {
        let mut amount_used_packet_ids: u64 = 0;
        let mut header = MessageHeader::new(MessageType::MEASUREMENT, test_id, packet_id);

        for i in 0..self.packets_amount {
            let start_of_packet = i * self.datagram_size as usize;
            header.set_packet_id(packet_id + amount_used_packet_ids);
            let serialized_header = header.serialize();
            let buffer = self.get_buffer_pointer();
            buffer[start_of_packet..(start_of_packet + serialized_header.len())].copy_from_slice(serialized_header);
            amount_used_packet_ids += 1;
        }
        debug!("Added packet IDs to buffer! Used packet IDs: {}, Next packet ID: {}", amount_used_packet_ids, packet_id + amount_used_packet_ids);
        // Return amount of used packet IDs
        Ok(amount_used_packet_ids)
    }

    fn create_msghdr(iov: &mut libc::iovec) -> libc::msghdr {
        let mut msghdr: libc::msghdr = unsafe { MaybeUninit::zeroed().assume_init() };
        
        msghdr.msg_name = std::ptr::null_mut();
        msghdr.msg_namelen = 0;
        msghdr.msg_iov = iov as *mut _;
        msghdr.msg_iovlen = 1;
        msghdr.msg_control = std::ptr::null_mut();
        msghdr.msg_controllen = 0;
    
        msghdr
    }

    pub fn set_address(&mut self, address: libc::sockaddr_in) {
        self.sockaddr = address;
        self.msghdr.msg_name = (&mut self.sockaddr) as *mut _ as *mut libc::c_void;
        self.msghdr.msg_namelen = std::mem::size_of::<libc::sockaddr_in>() as u32;
    }

    pub fn add_cmsg_buffer(&mut self) {
        self.with_cmsg = true;
        let msg_control = Box::leak(Box::new([0_u8; crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER]));
        self.msghdr.msg_control = msg_control as *mut _ as *mut libc::c_void;
        self.msghdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
    }

    fn create_iovec(buffer: &mut [u8]) -> &mut libc::iovec {
        Box::leak(Box::new(libc::iovec {
            iov_base: buffer.as_mut_ptr() as *mut _,
            iov_len: buffer.len(),
        }))
    }

    pub fn copy_buffer(&mut self, buffer: &[u8]) {
        if buffer.len() <= self.buffer_length {
            self.buffer_length = buffer.len();
            let buf = unsafe { (*self.msghdr.msg_iov).iov_base as *mut u8 };
            // Copy buffer into msghdr iovec
            unsafe { buf.copy_from(buffer.as_ptr(), buffer.len()) };
        }
    }

    pub fn get_msghdr(&mut self) -> &mut libc::msghdr {
        if self.with_cmsg {
            // Has to be set, since recvmsg overwrites this value 
            self.msghdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
        }
        self.msghdr.msg_flags = 0;

        &mut self.msghdr
    }

    pub fn move_msghdr(mut self) -> libc::msghdr {
        if self.with_cmsg {
            // Has to be set, since recvmsg overwrites this value 
            self.msghdr.msg_controllen = crate::LENGTH_MSGHDR_CONTROL_MESSAGE_BUFFER;
        }
        self.msghdr.msg_flags = 0;

        self.msghdr
    }

    pub fn get_buffer_pointer(&mut self) -> &mut [u8] {
        let iov_base = unsafe { (*self.msghdr.msg_iov).iov_base as *mut u8 };
        let iov_len = unsafe { (*self.msghdr.msg_iov).iov_len };
        unsafe { std::slice::from_raw_parts_mut(iov_base, iov_len) }
    }
}