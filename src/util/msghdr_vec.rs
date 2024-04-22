use super::msghdr::WrapperMsghdr;


pub struct MsghdrVec {
    pub msghdr_vec: Vec<WrapperMsghdr>,
    datagram_size: usize, // ASSUMPTION: It's the same for all msghdrs
    packets_amount_per_msghdr: usize // ASSUMPTION: It's the same for all msghdrs
}

impl MsghdrVec {
    pub fn new(size: usize, mss: u32, datagram_size: usize) -> MsghdrVec {
        let msghdr_vec = Vec::from_iter((0..size).map(|_| WrapperMsghdr::new(mss, datagram_size as u32).expect("Error creating packet buffer")));
        let packets_amount_per_msghdr = msghdr_vec.first().unwrap().packets_amount;

        MsghdrVec {
            msghdr_vec,
            datagram_size,
            packets_amount_per_msghdr
        }
    }

    pub fn with_cmsg_buffer(mut self) -> MsghdrVec {
        self.msghdr_vec.iter_mut().for_each(|msghdr| msghdr.add_cmsg_buffer());
        self
    }

    pub fn with_message_header(mut self, test_id: u64) -> MsghdrVec {
        for msghdr in self.msghdr_vec.iter_mut() {
            msghdr.add_message_header(test_id, 0).expect("Error adding message header");
        }
        self
    }

    pub fn with_random_payload(mut self) -> MsghdrVec {
        for msghdr in self.msghdr_vec.iter_mut() {
            msghdr.fill_with_repeating_pattern();
        }
        self
    }

    pub fn with_target_address(mut self, sockaddr: libc::sockaddr_in) -> MsghdrVec {
        self.msghdr_vec.iter_mut().for_each(|wrapper_msghdr| wrapper_msghdr.set_address(sockaddr));
        self
    }


    pub fn datagram_size(&self) -> usize {
        self.datagram_size
    }

    pub fn packets_amount_per_msghdr(&self) -> usize {
        self.packets_amount_per_msghdr
    }
}