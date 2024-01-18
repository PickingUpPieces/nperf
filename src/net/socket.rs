
use log::{info, trace, debug, error, warn};
use std::{self, net::Ipv4Addr};

pub struct Socket {
    ip: Ipv4Addr,
    port: u16,
    pub mtu_size: usize,
    socket: i32,
} 

impl Socket {
    pub fn new(ip: Ipv4Addr, port: u16, mtu_size: usize) -> Option<Socket> {
        let socket = Self::create_socket()?; 

        Some(Socket {
            ip,
            port,
            mtu_size,
            socket,
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

    pub fn send(&self, buffer: &[u8], buffer_len: usize) -> Result<(), &'static str> {
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
    
    pub fn recv(&self, buffer: &mut [u8]) -> Result<isize, &'static str> {
        let recv_result: isize = unsafe {
            // FIXME: Use read() like in iPerf
            libc::recv(
                self.socket,
                buffer.as_mut_ptr() as *mut _,
                buffer.len(),
                0
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

    pub fn set_nonblocking(&self) -> Result<(), &'static str> {    
        let mut flags = unsafe { libc::fcntl(self.socket, libc::F_GETFL, 0) };
        if flags == -1 {
            return Err("Failed to get socket flags");
        }
    
        flags |= libc::O_NONBLOCK;
    
        let fcntl_result = unsafe { libc::fcntl(self.socket, libc::F_SETFL, flags) };
        if fcntl_result == -1 {
            return Err("Failed to set socket flags");
        }
    
        Ok(())
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

    pub fn set_send_buffer_size(&self, size: u32) -> Result<(), &'static str> {
        let size_len = std::mem::size_of_val(&size) as libc::socklen_t;
        let current_size = Self::get_send_buffer_size(self.socket)?;
    
        if current_size >= size {
            warn!("New buffer size is smaller than current buffer size");
            return Ok(());
        }
    
        let setsockopt_result = unsafe {
            libc::setsockopt(
                self.socket,
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                &size as *const _ as _,
                size_len
            )
        };
    
        if setsockopt_result == -1 {
            error!("errno when setting send buffer size: {}", unsafe { *libc::__errno_location() });
            return Err("Failed to set socket send buffer size");
        }
    
        // TODO: Check only for okay with if let
        match Self::get_send_buffer_size(self.socket) {
            Ok(x) => {
                if x == size {
                    info!("New socket send buffer size: {}", x);
                    Ok(())
                } else {
                    error!("Current buffer size not equal desired one: {} vs {}", x, size);
                    Err("Failed to set socket send buffer size")
                }
            },
            Err(x) => {
                error!("{x}");
                Err("Failed to get new socket send buffer size")
            }
        }
    }
    
    fn get_send_buffer_size(socket: i32) -> Result<u32, &'static str> {
        let current_size: u32 = 0;
        let mut size_len = std::mem::size_of_val(&current_size) as libc::socklen_t;
    
        let getsockopt_result = unsafe {
            libc::getsockopt(
                socket,
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                &current_size as *const _ as _,
                &mut size_len as *mut _
            )
        };
    
        if getsockopt_result == -1 {
            error!("errno when getting send buffer size: {}", unsafe { *libc::__errno_location() });
            Err("Failed to get current socket send buffer size")
        } else {
            Ok(current_size)
        }
    }
    
    pub fn set_receive_buffer_size(&self, size: u32) -> Result<(), &'static str> {
        let size_len = std::mem::size_of::<u32>() as libc::socklen_t;
        let current_size = Self::get_receive_buffer_size(self.socket)?; 
    
        if current_size >= size {
            warn!("New buffer size is smaller than current buffer size");
            return Ok(());
        }
    
        // Set bigger buffer size
        let setsockopt_result = unsafe {
            libc::setsockopt(
                self.socket,
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                &size as *const _ as _,
                size_len
            )
        };
    
        if setsockopt_result == -1 {
            error!("errno when setting receive buffer size: {}", unsafe { *libc::__errno_location() });
            return Err("Failed to set socket receive buffer size");
        }
    
        match Self::get_receive_buffer_size(self.socket) {
            Ok(x) => {
                info!("New socket receive buffer size: {}", x);
                Ok(())
            },
            Err(x) => {
                error!("{x}");
                Err("Failed to get new socket receive buffer size")
            }
        }
    }
    
    
    fn get_receive_buffer_size(socket: i32) -> Result<u32, &'static str> {
        let current_size: u32 = 0;
        let mut size_len = std::mem::size_of_val(&current_size) as libc::socklen_t;
    
        let getsockopt_result = unsafe {
            libc::getsockopt(
                socket,
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                &current_size as *const _ as _,
                &mut size_len as *mut _
            )
        };
    
        if getsockopt_result == -1 {
            error!("errno when getting receive buffer size: {}", unsafe { *libc::__errno_location() });
            Err("Failed to get socket receive buffer size")
        } else {
            Ok(current_size)
        }
    }
}