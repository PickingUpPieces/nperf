
use log::{info, trace, debug, error};
use std::{self, net::Ipv4Addr};
use std::io::Error;

use super::socket_options::SocketOptions;

#[derive(Debug)]
pub struct Socket {
    ip: Ipv4Addr,
    port: u16,
    pub mtu_size: usize,
    socket: i32,
    _socket_options: SocketOptions,
} 

impl Socket {
    pub fn new(ip: Ipv4Addr, port: u16, mtu_size: usize, mut _socket_options: SocketOptions) -> Option<Socket> {
        let socket = Self::create_socket()?; 

        _socket_options.update(socket).expect("Error updating socket options");

        Some(Socket {
            ip,
            port,
            mtu_size,
            socket,
            _socket_options, 
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
            libc::send(
                self.socket,
                buffer.as_ptr() as *const _,
                buffer_len as usize,
                0
            )
        };
    
        if send_result == -1 {
            // CHeck for connection refused
            let errno = Error::last_os_error();
            let raw_os_err = errno.raw_os_error().expect("Failed to get raw os error");
            match raw_os_err {
                libc::ECONNREFUSED => {
                    error!("Connection refused while trying to send data!");
                    return Err("ECONNREFUSED");
                },
                libc::EMSGSIZE => {
                    error!("EMSGSIZE while trying to send data! The message is too large for the transport protocol.");
                    return Err("EMSGSIZE");
                },
                _ => {
                    error!("Errno when trying to send data: {}", errno);
                    return Err("Failed to send data");
                }
            }
        }
    
        debug!("Sent datagram with {} bytes", send_result);
        Ok(())
    }

    pub fn sendmsg(&self, msghdr: &libc::msghdr) -> Result<(), &'static str> {
        debug!("Trying to send message with msghdr length: {}", msghdr.msg_iovlen);

        let send_result = unsafe {
            libc::sendmsg(
                self.socket,
                msghdr as *const _ as _,
                0
            )
        };
    
        if send_result == -1 {
            let errno = Error::last_os_error();
            error!("Errno when trying to send data: {}", errno);

            let raw_os_err = errno.raw_os_error().expect("Failed to get raw os error");
            // Check for connection refused
            if raw_os_err == libc::ECONNREFUSED {
                return Err("ECONNREFUSED");
            } else {
                return Err("Failed to send data");
            }
        }
    
        debug!("Sent datagram(s) with {} bytes", send_result);
        Ok(())
    }

    pub fn recvmsg(&self, msghdr: &mut libc::msghdr) -> Result<isize, &'static str> {
        let recv_result: isize = unsafe {
            libc::recvmsg(
                self.socket,
                msghdr as *const _ as _,
                0
            )
        };
    
        if recv_result == -1 {
            // Check for non-blocking mode
            let errno = Error::last_os_error();
            let raw_os_err = errno.raw_os_error().expect("Failed to get raw os error");
            if raw_os_err == libc::EAGAIN {
                return Err("EAGAIN");
            } else {
                error!("Errno when trying to receive data with recvmsg(): {}", errno);
                return Err("Failed to receive data!");
            }
        } 
    
        debug!("Received {} bytes", recv_result);
        Ok(recv_result)
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
            let errno = Error::last_os_error();
            let raw_os_err = errno.raw_os_error().expect("Failed to get raw os error");
            if raw_os_err == libc::EAGAIN {
                return Err("EAGAIN");
            } else {
                error!("Errno when trying to receive data with read(): {}", errno);
                return Err("Failed to receive data");
            }
        } 
    
        debug!("Received {} bytes", recv_result);
        Ok(recv_result)
    }

    pub fn wait_for_data(&self) -> Result<(), &'static str> {
        debug!("Waiting for data on socket {}...", self.socket);
        // Prepare the file descriptor set for select
        let mut read_fds = unsafe {
            let mut fds: libc::fd_set = std::mem::zeroed();
            libc::FD_SET(self.socket, &mut fds);
            fds
        };
        
        // Call select
        let nfds = self.socket + 1;
        let result = unsafe { libc::select(nfds, &mut read_fds, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
        
        if result == -1 {
            let err = std::io::Error::last_os_error();
            error!("Error calling select: {}", err);
            return Err("Error calling select");
        }
        Ok(())
    }

    pub fn get_mtu(&self) -> Result<u32, &'static str> {
        self._socket_options.get_mtu(self.socket)
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
}