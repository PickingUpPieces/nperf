use log::{error, info, debug, warn};
use serde::Serialize;
use std::io::Error;

#[derive(PartialEq, Debug, Clone, Copy, Serialize)]
pub struct SocketOptions {
    nonblocking: bool,
    ip_fragmentation: bool,
    gso: (bool, u32),
    gro: bool,
    recv_buffer_size: u32,
    send_buffer_size: u32,
}

impl SocketOptions {
    pub fn new(nonblocking: bool, ip_fragmentation: bool, gso: (bool, u32), gro: bool, recv_buffer_size: u32, send_buffer_size: u32) -> Self {
        SocketOptions {
            nonblocking,
            ip_fragmentation,
            gso,
            gro,
            recv_buffer_size,
            send_buffer_size,
        }
    }

    pub fn update(&mut self, socket: i32) -> Result<(), &'static str> {
        debug!("Updating socket options with {:?}", self);
        if self.nonblocking {
            self.set_nonblocking(socket)?;
        } 
        if !self.ip_fragmentation {
            self.set_ip_fragmentation_off(socket)?;
        } 
        if self.gso.0 {
            self.set_gso(socket, self.gso.1)?;
        }
        if self.gro {
            self.set_gro(socket)?;
        }

        Self::set_receive_buffer_size(self, socket, self.recv_buffer_size)?;
        Self::set_send_buffer_size(self, socket, self.send_buffer_size)?;
        Ok(())
    }

    fn set_socket_option(socket: i32, level: libc::c_int, name: libc::c_int, value: u32) -> Result<(), &'static str> {
        let value_len = std::mem::size_of_val(&value) as libc::socklen_t;

        let setsockopt_result = unsafe {
            libc::setsockopt(
                socket,
                level,
                name,
                &value as *const _ as _,
                value_len 
            )
        };

        if setsockopt_result == -1 {
            error!("errno when enabling socket option on socket: {}", Error::last_os_error());
            return Err("Failed to enable socket option");
        }

        debug!("Enabled socket option on socket with value {}", value);
        Ok(())
    }

    fn get_socket_option(socket: i32, level: libc::c_int, name: libc::c_int) -> Result<u32, &'static str> {
        let ret = 0;
        let mut ret_len = std::mem::size_of_val(&ret) as libc::socklen_t;

        let getsockopt_result = unsafe {
            libc::getsockopt(
                socket,
                level,
                name,
                &ret as *const _ as _,
                &mut ret_len as *mut _
            )
        };
    
        if getsockopt_result == -1 {
            error!("errno when getting send buffer size: {}", Error::last_os_error());
            Err("Failed to get current socket send buffer size")
        } else {
            debug!("Got socket option on socket: {}", ret);
            Ok(ret)
        }
    }

    pub fn set_nonblocking(&mut self, socket: i32) -> Result<(), &'static str> {    
        let mut flags = unsafe { libc::fcntl(socket, libc::F_GETFL, 0) };
        if flags == -1 {
            return Err("Failed to get socket flags");
        }
    
        flags |= libc::O_NONBLOCK;
    
        let fcntl_result = unsafe { libc::fcntl(socket, libc::F_SETFL, flags) };
        if fcntl_result == -1 {
            return Err("Failed to set socket flags");
        }
        info!("Set socket to nonblocking mode");
        self.nonblocking = true;
        Ok(())
    }

    pub fn set_send_buffer_size(&mut self, socket: i32, size: u32) -> Result<(), &'static str> {
        let current_size = Self::get_send_buffer_size(socket)?;
        debug!("Trying to set send buffer size from {} to {}", current_size, size * 2);
    
        if current_size >= size * 2 {
            warn!("New buffer size {}*2 is smaller than current buffer size {}. Abort setting it...", size, current_size);
            return Ok(());
        }

        Self::set_socket_option(socket, libc::SOL_SOCKET, libc::SO_SNDBUF, size)
    }
    
    fn get_send_buffer_size(socket: i32) -> Result<u32, &'static str> {
        Self::get_socket_option(socket, libc::SOL_SOCKET, libc::SO_SNDBUF)
    }
    
    pub fn set_receive_buffer_size(&mut self, socket: i32, size: u32) -> Result<(), &'static str> {
        let current_size = Self::get_receive_buffer_size(socket)?; 
        debug!("Trying to set receive buffer size from {} to {}", current_size, size * 2);
    
        if current_size >= size * 2 {
            warn!("New buffer size {}*2 is smaller than current buffer size {}. Abort setting it...", size, current_size);
            return Ok(());
        }

        Self::set_socket_option(socket, libc::SOL_SOCKET, libc::SO_RCVBUF, size)
    }
    
    fn get_receive_buffer_size(socket: i32) -> Result<u32, &'static str> {
        Self::get_socket_option(socket, libc::SOL_SOCKET, libc::SO_RCVBUF)
    }

    pub fn set_gso(&mut self, socket: i32, gso_size: u32) -> Result<(), &'static str> {
        // gso_size should be equal to MSS = ETH_MSS - header(ipv4) - header(udp)
        info!("Set socket option GSO to {}", gso_size);
        Self::set_socket_option(socket, libc::SOL_UDP, libc::UDP_SEGMENT, gso_size)
    }

    pub fn set_gro(&mut self, socket: i32) -> Result<(), &'static str> {
        let value = 1;
        info!("Set socket option GRO to {}", value);
        Self::set_socket_option(socket, libc::SOL_UDP, libc::UDP_GRO, value)
    }

    pub fn set_ip_fragmentation_off(&mut self, socket: i32) -> Result<(), &'static str> {
        info!("Set socket to no IP fragmentation");
        Self::set_socket_option(socket, libc::IPPROTO_IP, libc::IP_MTU_DISCOVER, libc::IP_PMTUDISC_DO.try_into().unwrap())
    }

    pub fn get_mss(&self, socket: i32) -> Result<u32, &'static str> {
        // https://man7.org/linux/man-pages/man7/ip.7.html
        // MSS from TCP returned an error
        match Self::get_socket_option(socket, libc::IPPROTO_IP, libc::IP_MTU) {
            Ok(mtu) => Ok(mtu - 20 - 8), // Return MSS instead of MTU
            Err(_) => Err("Failed to get MSS")
        }
    }

    pub fn _get_gso_size(&self, socket: i32) -> Result<u32, &'static str> {
        Self::get_socket_option(socket, libc::SOL_UDP, libc::UDP_SEGMENT)
    }
}