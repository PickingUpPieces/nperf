
use log::{debug, error, info, trace, warn};
use std::{self, io::Error, mem::MaybeUninit, net::SocketAddrV4};

use super::socket_options::{self, SocketOptions};

#[derive(Debug, Copy, Clone)]
pub struct Socket {
    sock_addr_in: Option<SocketAddrV4>,
    sock_addr_out: Option<SocketAddrV4>,
    socket: i32,
    //sendmmsg_econnrefused_counter: u16
} 

impl Socket {
    pub fn new(mut socket_options: SocketOptions) -> Option<Socket> {
        let socket = Self::create_socket()?; 

        socket_options.set_socket_options(socket).expect("Error updating socket options");

        Some(Socket {
            sock_addr_in: None,
            sock_addr_out: None,
            socket,
            //sendmmsg_econnrefused_counter: 0
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


    pub fn connect(&mut self, sock_address: SocketAddrV4) -> Result<(), &'static str> {
        self.sock_addr_out = Some(sock_address);
        let sockaddr = Self::create_sockaddr(&self.sock_addr_out.expect("Outgoing socket address not set!"));
 
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

    pub fn bind(&mut self, sock_address: SocketAddrV4) -> Result<(), &'static str> {
        self.sock_addr_in = Some(sock_address);
        let sockaddr = Self::create_sockaddr(&self.sock_addr_in.expect("Outgoing socket address not set!"));
        debug!("Binding socket to {}:{}", sock_address, sockaddr.sin_port);
    
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
        }
        debug!("Sending on socket {} with buffer size: {}", self.socket, buffer_len);
        trace!("Buffer: {:?}", buffer);
    
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
                Some(libc::EAGAIN) => {
                    warn!("Error EAGAIN/EWOULDBLOCK: Probably socket buffer is full!");
                    return Err("EAGAIN");
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
                Some(libc::EAGAIN) => {
                    warn!("Error EAGAIN/EWOULDBLOCK: Probably socket buffer is full!");
                    return Err("EAGAIN");
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

    pub fn sendmmsg(&mut self, mmsgvec: &mut [libc::mmsghdr]) -> Result<usize, &'static str> {
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
                Some(libc::EAGAIN) => {
                    warn!("Error EGAIN/EWOULDBLOCK: Probably socket buffer is full!");
                    return Err("EAGAIN");
                },
                _ => {
                    error!("Errno when trying to send data with sendmmsg(): {}", errno);
                    return Err("Failed to send data");
                }
            }
        // sendmmsg() always returns 1, even when it should return ECONNREFUSED (when the server isn't up yet, similar to send()/sendmsg()). This is a workaround to detect ECONNREFUSED.
        // WARNING: sendmmsg_econnrefused_counter doesn't work under real load, because socket buffer is often full
        //} else if send_result == 1 && mmsgvec.len() > 1 {
        //    if self.sendmmsg_econnrefused_counter > 100 {
        //        error!("sendmmsg() returned 1, but mmsgvec.len() > 1. This probably means that the first message was sent successfully, but the second one failed. We assume that the server is not running.");
        //        return Err("ECONNREFUSED");
        //    } 
        //    self.sendmmsg_econnrefused_counter += 1;
        //} else {
        //    self.sendmmsg_econnrefused_counter = 0;
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
        debug!("Trying to receive message with msghdr length: {}, iov_len: {}, controllen: {}", msghdr.msg_iovlen, unsafe {*msghdr.msg_iov}.iov_len, msghdr.msg_controllen);
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
                // If no messages are available at the socket, the receive calls wait for a message to arrive, unless the socket is nonblocking (see fcntl(2)), in which case the value -1 is returned and the external variable errno is set to EAGAIN or EWOULDBLOCK.
                // From: https://linux.die.net/man/2/recvmsg
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
        socket_options::get_mss(self.socket)
    }

    pub fn get_socket_id(&self) -> i32 {
        self.socket
    }

    pub fn set_sock_addr_out(&mut self, sock_address: SocketAddrV4) {
        if self.sock_addr_out.is_some() {
            warn!("Overwriting existing socket address {} with {} on socket {}!", self.sock_addr_out.unwrap(), sock_address, self.socket);
        }

        self.sock_addr_out = Some(sock_address);
    }

    fn create_sockaddr(sock_address: &SocketAddrV4) -> libc::sockaddr_in {
        // Convert Ipv4Addr to libc::in_addr
        let addr = sock_address.ip(); 
        let addr_u32 = u32::from_le_bytes(addr.octets());
 
        #[cfg(target_os = "linux")]
        libc::sockaddr_in {
            sin_family: libc::AF_INET as u16,
            sin_port: sock_address.port().to_be(), // Convert to big endian
            sin_addr: libc::in_addr { s_addr: addr_u32 },
            sin_zero: [0; 8]
        }
    }

    #[allow(clippy::manual_map)]
    pub fn get_sockaddr_out(&self) -> Option<libc::sockaddr_in> {
        if let Some(sock_addr) = &self.sock_addr_out {
            Some(Self::create_sockaddr(sock_addr))
        } else {
            None
        }
    }

    pub unsafe fn create_fdset(&self) -> libc::fd_set {
        let mut fd_set: libc::fd_set = MaybeUninit::zeroed().assume_init();
        libc::FD_ZERO(&mut fd_set); 
        libc::FD_SET(self.socket, &mut fd_set);
        fd_set
    }

    pub fn create_pollfd(&self, event: libc::c_short) -> Vec<libc::pollfd> {
        let mut pollfd_vec: Vec<libc::pollfd> = Vec::new();
        let mut pollfd: libc::pollfd = unsafe { MaybeUninit::zeroed().assume_init() };
        pollfd.fd = self.socket;
        pollfd.events = event;
        pollfd_vec.push(pollfd);
        pollfd_vec
    }

    pub fn poll(&self, pollfd: &mut [libc::pollfd], timeout: i32) -> Result<(), &'static str> {
        let poll_result = unsafe {
            libc::poll(
                pollfd.as_mut_ptr(),
                pollfd.len() as u64,
                timeout 
            )
        };

        if poll_result == -1 {
            error!("Error occured executing poll(): {}", Error::last_os_error());
            Err("Error occured executing poll()")
        } else if poll_result == 0 {
            // Poll returned due to timeout
            warn!("Poll returned due to timeout");
            Err("TIMEOUT")
        } else {
            trace!("Poll returned with result: {}", poll_result);
            Ok(())
        }
    }

    pub fn select(&self, read_fds: Option<*mut libc::fd_set>, write_fds: Option<*mut libc::fd_set>, timeout: i32) -> Result<(), &'static str> {
        let nfds = self.socket + 1;
        let timeval =
            libc::timeval {
                tv_sec: timeout as i64 / 1000,
                tv_usec: (timeout as i64 % 1000) * 1000,
            };

        let result = unsafe { 
            libc::select(
                nfds, 
                read_fds.unwrap_or(std::ptr::null_mut()), 
                write_fds.unwrap_or(std::ptr::null_mut()), 
                std::ptr::null_mut(), 
                if timeout == -1 { std::ptr::null_mut() } else { &timeval as *const _ as *mut _ }
            ) 
        };
        if result == -1 {
            error!("Error calling select: {}", Error::last_os_error());
            Err("Error calling select")
        } else if result == 0 {
            debug!("Select returned due to timeout");
            Err("TIMEOUT")
        } else {
            debug!("Select returned with result: {}", result);
            Ok(())
        }
    }

}