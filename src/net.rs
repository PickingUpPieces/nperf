use libc::{self};
use log::{info, debug, error};
use std::{self, net::Ipv4Addr};
use std::str::FromStr;

pub fn bind_socket(socket: i32, address: Ipv4Addr, port: u16) -> Result<(), &'static str> {

    let sockaddr = create_sockaddr(address, port);

    let bind_result = unsafe {
        libc::bind(
            socket,
            &sockaddr as *const _ as _,
            std::mem::size_of_val(&sockaddr) as libc::socklen_t
        )
    };

    if bind_result == -1 {
        return Err("Failed to bind socket to port");
    }

    return Ok(())
}

pub fn create_socket() -> Result<i32, &'static str> {
    let socket = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if socket == -1 {
        return Err("Failed to create socket");
    }
    
    info!("Created socket: {:?}", socket);

    Ok(socket)
}

pub fn connect(socket: i32, address: Ipv4Addr, port: u16) -> Result<(), &'static str> {
    let sockaddr = create_sockaddr(address, port);

    let connect_result = unsafe {
        libc::connect(
            socket,
            &sockaddr as *const _ as _,
            std::mem::size_of_val(&sockaddr) as libc::socklen_t
        )
    };

    debug!("Connected to remote host with result: {:?}", connect_result);

    if connect_result == -1 {
        return Err("Failed to connect to remote host");
    }

    Ok(())
}


pub fn parse_ipv4(adress: String) -> Result<Ipv4Addr, &'static str> {
    match Ipv4Addr::from_str(adress.as_str()) {
        Ok(x) => Ok(x),
        Err(_) => Err("Invalid IPv4 address!"),
    }
}

fn create_sockaddr(address: Ipv4Addr, port: u16) -> libc::sockaddr_in {
    let addr_u32: u32 = address.into(); 

    #[cfg(target_os = "linux")]
    let addr = libc::sockaddr_in {
        sin_family: libc::AF_INET as u16,
        sin_port: port.to_be(), // Convert to big endian
        sin_addr: libc::in_addr { s_addr: addr_u32 },
        sin_zero: [0; 8]
    };

   #[cfg(target_os = "macos")]
   let addr = libc::sockaddr_in {
       sin_len: 8,
       sin_family: libc::AF_INET as u8,
       sin_port: port.to_be(), // Convert to big endian
       sin_addr: libc::in_addr { s_addr: addr_u32 },
       sin_zero: [0; 8]
   };

   debug!("Created sockaddr");

    addr
}

pub fn send(socket: i32, buffer: &[u8]) -> Result<(), &'static str> {
    if buffer.len() == 0 {
        error!("Buffer is empty");
        return Err("Buffer is empty");
    } else {
        debug!("Sending on socket {} with buffer size: {}", socket, buffer.len());
        debug!("Buffer: {:?}", buffer)
    }

    let send_result = unsafe {
        libc::send(
            socket,
            buffer.as_ptr() as *const _,
            buffer.len(),
            0
        )
    };

    if send_result == -1 {
        return Err("Failed to send data");
    }

    debug!("Sent {} bytes", send_result);

    Ok(())
}

pub fn recv(socket: i32, buffer: &mut [u8]) -> Result<(), &'static str> {
    let recv_result = unsafe {
        libc::recv(
            socket,
            buffer.as_mut_ptr() as *mut _,
            buffer.len(),
            0
        )
    };

    if recv_result == -1 {
        return Err("Failed to receive data");
    }

    debug!("Received {} bytes", recv_result);

    Ok(())
}