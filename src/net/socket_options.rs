use log::{error, info, debug};
use serde::Serialize;
use std::{fmt::Display, io::Error};
use crate::util::statistic::serialize_option_as_bool;


#[derive(PartialEq, Debug, Clone, Copy, Serialize)]
pub struct SocketOptions {
    nonblocking: bool,
    ip_fragmentation: bool,
    reuseport: bool,
    #[serde(with = "serialize_option_as_bool")]
    gso: Option<u32>,
    pub gro: bool,
    pub socket_pacing_rate: u64,
    #[serde(with = "serialize_option_as_bool")]
    recv_buffer_size: Option<u32>,
    #[serde(with = "serialize_option_as_bool")]
    send_buffer_size: Option<u32>,

}

impl SocketOptions {
    #[allow(clippy::too_many_arguments)]
    pub fn new(nonblocking: bool, ip_fragmentation: bool, reuseport: bool, gso: Option<u32>, gro: bool, socket_pacing_rate: u64, recv_buffer_size: Option<u32>, send_buffer_size: Option<u32>) -> Self {
        SocketOptions {
            nonblocking,
            ip_fragmentation,
            reuseport,
            gso,
            gro,
            socket_pacing_rate,
            recv_buffer_size,
            send_buffer_size,
        }
    }

    pub fn set_socket_options(&mut self, socket: i32) -> Result<(), &'static str> {
        debug!("Updating socket options with {:?}", self);
        set_reuseport(socket, self.reuseport)?;

        if self.nonblocking {
            set_nonblocking(socket)?;
        } 
        if !self.ip_fragmentation {
            set_ip_fragmentation_off(socket)?;
        } 
        if let Some(size) = self.gso {
            set_gso(socket, size)?;
        }

        if self.socket_pacing_rate > 0 {
            set_socket_pacing(socket, self.socket_pacing_rate)?;
        }

        set_gro(socket, self.gro)?;

        if let Some(size) = self.send_buffer_size { 
            set_buffer_size(socket, size, libc::SO_SNDBUF)?;
        } else {
            self.send_buffer_size = Some(get_socket_option(socket, libc::SOL_SOCKET, libc::SO_SNDBUF)?);
        }

        if let Some(size) = self.recv_buffer_size { 
            set_buffer_size(socket, size, libc::SO_RCVBUF)?;
        } else {
            self.recv_buffer_size = Some(get_socket_option(socket, libc::SOL_SOCKET, libc::SO_RCVBUF)?); 
        }
        Ok(())
    }
}


fn set_socket_option<T: Display>(socket: i32, level: libc::c_int, name: libc::c_int, value: T) -> Result<(), &'static str> {
    let value_len = std::mem::size_of::<T>() as libc::socklen_t;

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
        error!("errno when getting socket option: {}", Error::last_os_error());
        Err("Failed to get socket option")
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

    match set_socket_option(socket, libc::SOL_SOCKET, buffer_type, size) {
        Ok(_) => {
            current_size = get_socket_option(socket, libc::SOL_SOCKET, buffer_type)?; 
            if current_size < size * 2 {
                Err(format!("Planned buffer size {} (Buffer size is always allocated times 2 by linux) is smaller than current buffer size {} after trying to set it. Setting buffer size failed...", size * 2, current_size).leak())
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
    set_socket_option(socket, libc::IPPROTO_IP, libc::IP_MTU_DISCOVER, libc::IP_PMTUDISC_DO)
}

pub fn get_mss(socket: i32) -> Result<u32, &'static str> {
    // https://man7.org/linux/man-pages/man7/ip.7.html
    // MSS from TCP returned an error
    match get_socket_option(socket, libc::IPPROTO_IP, libc::IP_MTU) {
        Ok(mtu) => Ok(mtu - 20 - 8), // Return MSS instead of MTU
        Err(_) => Err("Failed to get MSS")
    }
}

pub fn set_socket_pacing(socket: i32, pacing_rate: u64) -> Result<(), &'static str> {
    info!("Set socket option pacing to for current socket to {}B/s", pacing_rate);
    set_socket_option(socket, libc::SOL_SOCKET, libc::SO_MAX_PACING_RATE, pacing_rate)
}

pub fn _get_gso_size(socket: i32) -> Result<u32, &'static str> {
    get_socket_option(socket, libc::SOL_UDP, libc::UDP_SEGMENT)
}

pub fn set_reuseport(socket: i32, status: bool) -> Result<(), &'static str> {
    let value: u32 = if status { 1 } else { 0 };
    info!("Set socket option REUSEPORT to {}", status);
    set_socket_option(socket, libc::SOL_SOCKET, libc::SO_REUSEPORT, value)
}