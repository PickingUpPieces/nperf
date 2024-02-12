
use log::{info, trace, debug, error};
use std::{self, net::Ipv4Addr};
use std::io::Error;

use super::socket_options::SocketOptions;

#[derive(Debug)]
pub struct Socket {
    ip: Ipv4Addr,
    port: u16,
    socket: i32,
    socket_options: SocketOptions,
} 

impl Socket {
    pub fn new(ip: Ipv4Addr, port: u16, mut socket_options: SocketOptions) -> Option<Socket> {
        let socket = Self::create_socket()?; 

        socket_options.update(socket).expect("Error updating socket options");

        Some(Socket {
            ip,
            port,
            socket,
            socket_options, 
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
    
        Ok(())
    }

    pub fn close(&self) -> Result<(), &'static str> {
        let close_result = unsafe {
            libc::close(self.socket)
        };
    
        if close_result == -1 {
            return Err("Failed to close socket");
        }
    
        Ok(())
    }

    pub fn send(&self, buffer: &[u8], buffer_len: usize) -> Result<usize, &'static str> {
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
                buffer_len,
                0
            )
        };
    
        if send_result <= -1 {
            // CHeck for connection refused
            let errno = Error::last_os_error();
            match errno.raw_os_error() {
                Some(libc::ECONNREFUSED) => {
                    error!("Connection refused while trying to send data!");
                    return Err("ECONNREFUSED");
                },
                Some(libc::EMSGSIZE) => {
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
        Ok(send_result as usize)
    }

    pub fn sendmsg(&self, msghdr: &libc::msghdr) -> Result<usize, &'static str> {
        debug!("Trying to send message with msghdr length: {}, iov_len: {}", msghdr.msg_iovlen, unsafe {*msghdr.msg_iov}.iov_len);
        trace!("Trying to send message with iov_buffer: {:?}", unsafe { std::slice::from_raw_parts((*msghdr.msg_iov).iov_base as *const u8, (*msghdr.msg_iov).iov_len)});

        let send_result = unsafe {
            libc::sendmsg(
                self.socket,
                msghdr as *const _ as _,
                0
            )
        };
    
        if send_result <= -1 {
            let errno = Error::last_os_error();
            match errno.raw_os_error() {
                Some(libc::ECONNREFUSED) => {
                    error!("Connection refused while trying to send data!");
                    return Err("ECONNREFUSED");
                },
                _ => {
                    error!("Errno when trying to send data with sendmsg(): {}", errno);
                    return Err("Failed to send data");
                }
            }
        }
    
        debug!("Sent datagram(s) with {} bytes", send_result);
        Ok(send_result as usize)
    }

    pub fn sendmmsg(&self, mmsgvec: &mut [libc::mmsghdr]) -> Result<usize, &'static str> {
        let send_result: i32 = unsafe {
            libc::sendmmsg(
                self.socket,
                mmsgvec.as_mut_ptr(),
                mmsgvec.len() as u32,
                0
            )
        };
    
        if send_result <= -1 {
            let errno = Error::last_os_error();
            match errno.raw_os_error() {
                Some(libc::ECONNREFUSED) => {
                    error!("Connection refused while trying to send data!");
                    return Err("ECONNREFUSED");
                },
                _ => {
                    error!("Errno when trying to send data with sendmmsg(): {}", errno);
                    return Err("Failed to send data");
                }
            }
        } else if send_result == 1 && mmsgvec.len() > 1 {
            error!("sendmmsg() returned 1, but mmsgvec.len() > 1. This probably means that the first message was sent successfully, but the second one failed. We assume that the server is not running.");
            return Err("ECONNREFUSED");
        }
    
        debug!("Sent {} mmsghdr(s)", send_result);
        Ok(send_result as usize)
    }

    pub fn recvmmsg(&self, msgvec: &mut [libc::mmsghdr]) -> Result<usize, &'static str> {
        let timeout = std::ptr::null::<libc::timespec>() as *mut libc::timespec;

        let recv_result: i32 = unsafe {
            libc::recvmmsg(
                self.socket,
                msgvec.as_mut_ptr(),
                msgvec.len() as u32,
                0,
                timeout
            )
        };
    
        if recv_result <= -1 {
            let errno = Error::last_os_error();
            match errno.raw_os_error() {
                Some(libc::EAGAIN) => {
                    return Err("EAGAIN");
                },
                _ => {
                    error!("Errno when trying to receive data with recvmmsg(): {}", errno);
                    return Err("Failed to receive data!");
                }
            }
        } 
    
        debug!("Received {} mmsghdr(s)", recv_result);
        Ok(recv_result as usize)
    }

    pub fn recvmsg(&self, msghdr: &mut libc::msghdr) -> Result<usize, &'static str> {
        debug!("Trying to receive message with msghdr length: {}, iov_len: {}", msghdr.msg_iovlen, unsafe {*msghdr.msg_iov}.iov_len);
        trace!("Trying to receive message with iov_buffer: {:?}", unsafe { std::slice::from_raw_parts((*msghdr.msg_iov).iov_base as *const u8, (*msghdr.msg_iov).iov_len)});

        let recv_result: isize = unsafe {
            libc::recvmsg(
                self.socket,
                msghdr as *mut _ as _,
                0
            )
        };
    
        if recv_result <= -1 {
            let errno = Error::last_os_error();
            match errno.raw_os_error() {
                Some(libc::EAGAIN) => {
                    return Err("EAGAIN");
                },
                _ => {
                    error!("Errno when trying to receive data with recvmsg(): {}", errno);
                    return Err("Failed to receive data!");
                }
            }
        } 
    
        debug!("Received {} bytes", recv_result);
        Ok(recv_result as usize)
    }
    
    pub fn recv(&self, buffer: &mut [u8]) -> Result<usize, &'static str> {
        let recv_result: isize = unsafe {
            libc::recv(
                self.socket,
                buffer.as_mut_ptr() as *mut _,
                buffer.len(),
                0
            )
        };

        if recv_result <= -1 {
            let errno = Error::last_os_error();
            match errno.raw_os_error() {
                Some(libc::EAGAIN) => {
                    return Err("EAGAIN");
                },
                _ => {
                    error!("Errno when trying to receive data with recv(): {}", errno);
                    return Err("Failed to receive data!");
                }
            }
        } 
    
        debug!("Received {} bytes", recv_result);
        Ok(recv_result as usize)
    }

    pub fn get_mss(&self) -> Result<u32, &'static str> {
        self.socket_options.get_mss(self.socket)
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

    pub unsafe fn create_fdset(&self) -> libc::fd_set {
        let mut fd_set: libc::fd_set =std::mem::zeroed();
        libc::FD_ZERO(&mut fd_set); 
        libc::FD_SET(self.socket, &mut fd_set);
        fd_set
    }

    pub fn create_pollfd(&self, event: libc::c_short) -> Vec<libc::pollfd> {
        let mut pollfd_vec: Vec<libc::pollfd> = Vec::new();
        let mut pollfd: libc::pollfd = unsafe { std::mem::zeroed() };
        pollfd.fd = self.socket;
        pollfd.events = event;
        pollfd_vec.push(pollfd);
        pollfd_vec
    }

    pub fn poll(&self, pollfd: &mut [libc::pollfd]) -> Result<(), &'static str> {
        let poll_result = unsafe {
            libc::poll(
                pollfd.as_mut_ptr(),
                pollfd.len() as u64,
                -1
            )
        };

        if poll_result == -1 {
            error!("Error calling poll: {}", Error::last_os_error());
            Err("Error calling poll")
        } else {
            trace!("Poll returned with result: {}", poll_result);
            Ok(())
        }
    }

    pub fn select(&self, read_fds: Option<*mut libc::fd_set>, write_fds: Option<*mut libc::fd_set>) -> Result<(), &'static str> {
        let nfds = self.socket + 1;
        let result = unsafe { libc::select(nfds, read_fds.unwrap_or(std::ptr::null_mut()), write_fds.unwrap_or(std::ptr::null_mut()), std::ptr::null_mut(), std::ptr::null_mut()) };
        
        if result == -1 {
            error!("Error calling select: {}", Error::last_os_error());
            Err("Error calling select")
        } else {
            debug!("Select returned with result: {}", result);
            Ok(())
        }
    }
}