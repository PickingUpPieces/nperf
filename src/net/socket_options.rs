use log::{error, info, debug, warn};
use serde::Serialize;
use std::io::Error;


#[derive(PartialEq, Debug, Clone, Copy, Serialize)]
pub struct SocketOptions {
    nonblocking: bool,
    ip_fragmentation: bool,
    reuseport: bool,
    gso: Option<u32>,
    gro: bool,
    recv_buffer_size: Option<u32>,
    send_buffer_size: Option<u32>,
}

impl SocketOptions {
    pub fn new(nonblocking: bool, ip_fragmentation: bool, reuseport: bool, gso: Option<u32>, gro: bool, recv_buffer_size: Option<u32>, send_buffer_size: Option<u32>) -> Self {
        SocketOptions {
            nonblocking,
            ip_fragmentation,
            reuseport,
            gso,
            gro,
            recv_buffer_size,
            send_buffer_size,
        }
    }

    pub fn set_socket_options(&mut self, socket: i32) -> Result<(), &'static str> {
        debug!("Updating socket options with {:?}", self);
        if self.nonblocking {
            set_nonblocking(socket)?;
        } 
        if !self.ip_fragmentation {
            set_ip_fragmentation_off(socket)?;
        } 
        if let Some(size) = self.gso {
            set_gso(socket, size)?;
        }

        set_gro(socket, self.gro)?;
        set_reuseport(socket, self.reuseport)?;

        if let Some(size) = self.recv_buffer_size { 
            set_buffer_size(socket, size, libc::SO_SNDBUF)?;
        }

        if let Some(size) = self.recv_buffer_size { 
            set_buffer_size(socket, size, libc::SO_RCVBUF)?;
        }
        Ok(())
    }
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

pub fn set_nonblocking(socket: i32) -> Result<(), &'static str> {    
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
    Ok(())
}

pub fn set_buffer_size(socket: i32, size: u32, buffer_type: libc::c_int) -> Result<(), &'static str> {
    let mut current_size = get_socket_option(socket, libc::SOL_SOCKET, buffer_type)?; 
    match buffer_type {
        libc::SO_SNDBUF => info!("Set send buffer size from {} to {}", current_size, size),
        libc::SO_RCVBUF => info!("Set receive buffer size from {} to {}", current_size, size),
        _ => return Err("Invalid buffer type")
    }

    if current_size >= size * 2 {
        warn!("New buffer size {} is smaller than current buffer size {}. Abort setting it...", size * 2, current_size);
        return Ok(());
    }

    match set_socket_option(socket, libc::SOL_SOCKET, buffer_type, size) {
        Ok(_) => {
            current_size = get_socket_option(socket, libc::SOL_SOCKET, buffer_type)?; 
            if current_size < size * 2 {
                Err(format!("Planned buffer size {} is smaller than current buffer size {}. Setting buffer size failed...", size * 2, current_size).leak())
            } else {
                Ok(())
            }
        },
        Err(e) => Err(e)
    }
}

fn set_gso(socket: i32, gso_size: u32) -> Result<(), &'static str> {
    // gso_size should be equal to MSS = ETH_MSS - header(ipv4) - header(udp)
    info!("Set socket option GSO to {}", gso_size);
    set_socket_option(socket, libc::SOL_UDP, libc::UDP_SEGMENT, gso_size)
}

fn set_gro(socket: i32, status: bool) -> Result<(), &'static str> {
    let value: u32 = if status { 1 } else { 0 };
    info!("Set socket option GRO to {}", status);
    set_socket_option(socket, libc::SOL_UDP, libc::UDP_GRO, value)
}

fn set_ip_fragmentation_off(socket: i32) -> Result<(), &'static str> {
    info!("Set socket to no IP fragmentation");
    set_socket_option(socket, libc::IPPROTO_IP, libc::IP_MTU_DISCOVER, libc::IP_PMTUDISC_DO.try_into().unwrap())
}

pub fn get_mss(socket: i32) -> Result<u32, &'static str> {
    // https://man7.org/linux/man-pages/man7/ip.7.html
    // MSS from TCP returned an error
    match get_socket_option(socket, libc::IPPROTO_IP, libc::IP_MTU) {
        Ok(mtu) => Ok(mtu - 20 - 8), // Return MSS instead of MTU
        Err(_) => Err("Failed to get MSS")
    }
}

pub fn _get_gso_size(socket: i32) -> Result<u32, &'static str> {
    get_socket_option(socket, libc::SOL_UDP, libc::UDP_SEGMENT)
}

pub fn set_reuseport(socket: i32, status: bool) -> Result<(), &'static str> {
    let value: u32 = if status { 1 } else { 0 };
    info!("Set socket option REUSEPORT to {}", status);
    set_socket_option(socket, libc::SOL_SOCKET, libc::SO_REUSEPORT, value)
}