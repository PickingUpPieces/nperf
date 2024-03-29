use log::debug;
use crate::net::{MessageHeader, MessageType};

const LENGTH_CONTROL_MESSAGE_BUFFER: usize = 100;

pub struct PacketBuffer {
    buffer: Vec<u8>,
    iov: libc::iovec,
    with_cmsg: bool,
    msg_control: [u8; LENGTH_CONTROL_MESSAGE_BUFFER],
    msghdr: libc::msghdr,
    sockaddr: libc::sockaddr_in,
    datagram_size: u32,
    _last_packet_size: u32,
    packets_amount: usize,
}

impl PacketBuffer {
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
        debug!("Created PacketBuffer with datagram size: {}, last packet size: {}, buffer length: {}, packets amount: {}", datagram_size, _last_packet_size, mss, packets_amount);

        let mut buffer = vec![0; mss as usize];
        let mut iov = libc::iovec {
            iov_base: buffer.as_mut_ptr() as *mut _,
            iov_len: buffer.len(),
        };

        let msg_control = [0; LENGTH_CONTROL_MESSAGE_BUFFER];
        let msghdr = Self::create_msghdr(&mut iov);
        let sockaddr: libc::sockaddr_in = unsafe { std::mem::zeroed() };

        Some(PacketBuffer {
            buffer,
            iov,
            with_cmsg: false,
            msg_control,
            msghdr,
            sockaddr,
            datagram_size,
            _last_packet_size,
            packets_amount,
        })
    }

    fn create_msghdr(iov: &mut libc::iovec) -> libc::msghdr {
        let mut msghdr: libc::msghdr = unsafe { std::mem::zeroed() };
        
        msghdr.msg_name = std::ptr::null_mut();
        msghdr.msg_namelen = 0;
        msghdr.msg_iov = iov as *mut _;
        msghdr.msg_iovlen = 1;
        msghdr.msg_control = std::ptr::null_mut();
        msghdr.msg_controllen = 0;
    
        msghdr
    }

    fn set_msghdr_iov(&mut self) {
        self.msghdr.msg_iov = &mut self.iov as *mut _;
    }

    pub fn set_address(&mut self, address: libc::sockaddr_in) {
        self.sockaddr = address;
        self.msghdr.msg_name = (&mut self.sockaddr) as *mut _ as *mut libc::c_void;
        self.msghdr.msg_namelen = std::mem::size_of::<libc::sockaddr_in>() as u32;
    }

    pub fn get_msghdr(&mut self) -> &mut libc::msghdr {
        // Re-set iov pointer, since it could have been reallocated
        self.set_msghdr_iov();
        if self.with_cmsg {
            self.add_cmsg_buffer();
        }
        &mut self.msghdr
    }

    pub fn add_cmsg_buffer(&mut self) {
        self.with_cmsg = true;
        self.msghdr.msg_control = (&mut self.msg_control) as *mut _ as *mut libc::c_void;
        self.msghdr.msg_controllen = self.msg_control.len();
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
    }

    pub fn add_message_header(&mut self, test_id: u64, packet_id: u64) -> Result<u64, &'static str> {
        let mut amount_used_packet_ids: u64 = 0;
        let mut header = MessageHeader::new(MessageType::MEASUREMENT, test_id, packet_id);

        for i in 0..self.packets_amount {
            let start_of_packet = i * self.datagram_size as usize;
            header.set_packet_id(packet_id + amount_used_packet_ids);
            let serialized_header = header.serialize();
            self.buffer[start_of_packet..(start_of_packet + serialized_header.len())].copy_from_slice(serialized_header);
            amount_used_packet_ids += 1;
        }
        debug!("Added packet IDs to buffer! Used packet IDs: {}, Next packet ID: {}", amount_used_packet_ids, packet_id + amount_used_packet_ids);
        // Return amount of used packet IDs
        Ok(amount_used_packet_ids)
    }

    pub fn add_packet_ids(&mut self, packet_id: u64) -> Result<u64, &'static str> {
        let mut amount_used_packet_ids: u64 = 0;

        for i in 0..self.packets_amount {
            let start_of_packet = i * self.datagram_size as usize;
            MessageHeader::set_packet_id_raw(&mut self.buffer[start_of_packet..], packet_id + amount_used_packet_ids);
            amount_used_packet_ids += 1;
        }

        debug!("Added packet IDs to buffer! Used packet IDs: {}, Next packet ID: {}", amount_used_packet_ids, packet_id + amount_used_packet_ids);
        // Return amount of used packet IDs
        Ok(amount_used_packet_ids)
    }


    pub fn get_buffer_pointer(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    pub fn set_buffer(&mut self, buffer: Vec<u8>) {
        self.buffer = buffer;
        let iov = libc::iovec {
            iov_base: self.buffer.as_mut_ptr() as *mut _,
            iov_len: self.buffer.len(),
        };
        self.iov = iov;
    }

    pub fn get_buffer_length(&self) -> usize {
        self.buffer.len()
    }

    pub fn get_packet_amount(&self) -> usize {
        self.packets_amount
    }

    pub fn get_datagram_size(&self) -> u32 {
        self.datagram_size
    }
}