use libc::{self};
use std::{self, net::Ipv4Addr};
use std::str::FromStr;

pub fn bind_socket(socket: i32, address: Ipv4Addr, port: u16) -> Result<(), &'static str> {

    let addr_u32: u32 = address.into(); 

   // pub struct sockaddr_in {
   //     pub sin_family: sa_family_t,
   //     pub sin_port: ::in_port_t,
   //     pub sin_addr: ::in_addr,
   //     pub sin_zero: [u8; 8],
   // }
    #[cfg(target_os = "linux")] // untested
    let addr = libc::sockaddr_in {
        sin_family: libc::AF_INET as u8,
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

    let bind_result = unsafe {
        libc::bind(
            socket,
            &addr as *const _ as _,
            std::mem::size_of_val(&addr) as libc::socklen_t
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
    
    println!("Created socket: {:?}", socket);

    Ok(socket)
}


pub fn parse_ipv4(adress: String) -> Result<Ipv4Addr, &'static str> {
    match Ipv4Addr::from_str(adress.as_str()) {
        Ok(x) => Ok(x),
        Err(_) => Err("Invalid IPv4 address!"),
    }
}