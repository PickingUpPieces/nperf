
use log::{info, trace, debug, error};
use std::{self, net::Ipv4Addr};

use super::socket_options::SocketOptions;

pub struct Socket {
    ip: Ipv4Addr,
    port: u16,
    pub mtu_size: usize,
    socket: i32,
    options: SocketOptions,
} 

impl Socket {
    pub fn new(ip: Ipv4Addr, port: u16, mtu_size: usize, use_gso: bool) -> Option<Socket> {
        let socket = Self::create_socket()?; 

        let options = SocketOptions::new(true, use_gso, false, crate::DEFAULT_SOCKET_RECEIVE_BUFFER_SIZE, crate::DEFAULT_SOCKET_SEND_BUFFER_SIZE);

        Some(Socket {
            ip,
            port,
            mtu_size,
            socket,
            options
        })
    }

    fn create_socket() -> Option<i32> {
        let socket = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if socket == -1 {
            error!("Failed to create socket");
            return None;
        }
        
        info!("Created socket: {:?}", socket);
        Some(socket)
    }


    pub fn connect(&self) -> Result<(), &'static str> {
        let sockaddr = Self::create_sockaddr(self.ip, self.port);
    
        let connect_result = unsafe {
            libc::connect(
                self.socket,
                &sockaddr as *const _ as _,
                std::mem::size_of_val(&sockaddr) as libc::socklen_t
            )
        };
        debug!("'Connected' to remote host with result: {:?}", connect_result);
    
        if connect_result == -1 {
            return Err("Failed to connect to remote host");
        }
    
        Ok(())
    }

    pub fn bind(&self) -> Result<(), &'static str> {
        let sockaddr = Self::create_sockaddr(self.ip, self.port);
    
        let bind_result = unsafe {
            libc::bind(
                self.socket,
                &sockaddr as *const _ as _,
                std::mem::size_of_val(&sockaddr) as libc::socklen_t
            )
        };
    
        if bind_result == -1 {
            return Err("Failed to bind socket to port");
        }
    
        return Ok(())
    }

    pub fn write(&self, buffer: &[u8], buffer_len: usize) -> Result<(), &'static str> {
        if buffer_len == 0 {
            error!("Buffer is empty");
            return Err("Buffer is empty");
        } else {
            debug!("Sending on socket {} with buffer size: {}", self.socket, buffer_len);
            trace!("Buffer: {:?}", buffer)
        }
    
        let send_result = unsafe {
            libc::write(
                self.socket,
                buffer.as_ptr() as *const _,
                buffer_len as usize
            )
        };
    
        if send_result == -1 {
            // CHeck for connection refused
            if unsafe { *libc::__errno_location() } == libc::ECONNREFUSED {
                error!("Connection refused while trying to send data!");
                return Err("ECONNREFUSED");
            }
            error!("Errno when trying to send data: {}", unsafe { *libc::__errno_location() });
            return Err("Failed to send data");
        }
    
        debug!("Sent datagram with {} bytes", send_result);
        Ok(())
    }
    
    pub fn read(&self, buffer: &mut [u8]) -> Result<isize, &'static str> {
        let recv_result: isize = unsafe {
            // FIXME: Use read() like in iPerf
            libc::read(
                self.socket,
                buffer.as_mut_ptr() as *mut _,
                buffer.len()
            )
        };
    
        if recv_result == -1 {
            // Check for non-blocking mode
            if unsafe { *libc::__errno_location() } == libc::EAGAIN {
                return Err("EAGAIN");
            } else {
                return Err("Failed to receive data");
            }
        } 
    
        debug!("Received {} bytes", recv_result);
        Ok(recv_result)
    }

    pub fn get_mtu(&self) -> Result<u32, &'static str> {
        // https://man7.org/linux/man-pages/man7/ip.7.html
        // MSS from TCP returned an error
        let current_size: u32 = 0;
        let mut size_len = std::mem::size_of_val(&current_size) as libc::socklen_t;

        // IP_MTU
        let getsockopt_result = unsafe {
            libc::getsockopt(
                self.socket,
                libc::IPPROTO_IP,
                libc::IP_MTU,
                &current_size as *const _ as _,
                &mut size_len as *mut _
            )
        };

        if getsockopt_result == -1 {
            error!("errno when getting Ethernet MTU: {}", unsafe { *libc::__errno_location() });
            Err("Failed to get socket Ethernet MTU")
        } else {
            info!("Current socket Ethernet MTU: {}", current_size);
            // Minus IP header size and UDP header size
            Ok(current_size - 20 - 8)
        }
    }

    fn create_sockaddr(address: Ipv4Addr, port: u16) -> libc::sockaddr_in {
        let addr_u32: u32 = address.into(); 
    
        #[cfg(target_os = "linux")]
        libc::sockaddr_in {
            sin_family: libc::AF_INET as u16,
            sin_port: port.to_be(), // Convert to big endian
            sin_addr: libc::in_addr { s_addr: addr_u32 },
            sin_zero: [0; 8]
        }
    }

    pub fn set_nonblocking(&mut self) -> Result<(), &'static str> {
        self.options.set_nonblocking(self.socket)
    }

    pub fn set_receive_buffer_size(&mut self, size: u32) -> Result<(), &'static str> {
        self.options.set_receive_buffer_size(self.socket, size)
    }

    pub fn set_send_buffer_size(&mut self, size: u32) -> Result<(), &'static str> {
        self.options.set_send_buffer_size(self.socket, size)
    }

    pub fn set_gso(&mut self) -> Result<(), &'static str> {
        self.options.set_gso(self.socket, self.mtu_size as u64)
    } 
}