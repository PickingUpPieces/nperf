use libc::{self};
use log::{info, debug, error, warn};
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

    debug!("'Connected' to remote host with result: {:?}", connect_result);

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
    libc::sockaddr_in {
        sin_family: libc::AF_INET as u16,
        sin_port: port.to_be(), // Convert to big endian
        sin_addr: libc::in_addr { s_addr: addr_u32 },
        sin_zero: [0; 8]
    }
}

pub fn send(socket: i32, buffer: &[u8], buffer_len: usize) -> Result<(), &'static str> {
    if buffer_len == 0 {
        error!("Buffer is empty");
        return Err("Buffer is empty");
    } else {
        debug!("Sending on socket {} with buffer size: {}", socket, buffer_len);
        debug!("Buffer: {:?}", buffer)
    }

    let start = std::time::Instant::now();

    let send_result = unsafe {
        libc::send(
            socket,
            buffer.as_ptr() as *const _,
            buffer_len as usize,
            0
        )
    };
    let duration = start.elapsed();
    if duration.as_micros() > 20 {
        warn!("Time elapsed in send() is: {:?}", duration);
    } 

    if send_result == -1 {
        // CHeck for connection refused
        if unsafe { *libc::__errno_location() } == libc::ECONNREFUSED {
            error!("Connection refused while trying to send data!");
            return Err("ECONNREFUSED");
        }
        return Err("Failed to send data");
    }

    debug!("Sent {} bytes", send_result);

    Ok(())
}

pub fn recv(socket: i32, buffer: &mut [u8]) -> Result<isize, &'static str> {
    let start = std::time::Instant::now();

    let recv_result: isize = unsafe {
        libc::recv(
            socket,
            buffer.as_mut_ptr() as *mut _,
            buffer.len(),
            0
        )
    };
    let duration = start.elapsed();
    if duration.as_micros() > 20 {
        warn!("Time elapsed in recv() is: {:?}", duration);
    } 

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

pub fn set_socket_nonblocking(socket: i32) -> Result<(), &'static str> {    
    let mut flags = unsafe { libc::fcntl(socket, libc::F_GETFL, 0) };
    if flags == -1 {
        return Err("Failed to get socket flags");
    }

    flags |= libc::O_NONBLOCK;

    let fcntl_result = unsafe { libc::fcntl(socket, libc::F_SETFL, flags) };
    if fcntl_result == -1 {
        return Err("Failed to set socket flags");
    }

    Ok(())
}

pub fn set_socket_send_buffer_size(socket: i32, size: u32) -> Result<(), &'static str> {
    let size_len = std::mem::size_of::<u32>() as libc::socklen_t;

    let current_size = match get_socket_send_buffer_size(socket) {
        Ok(x) => {
            info!("Current socket send buffer size: {}", x);
            x
        },
        Err(x) => {
            error!("{x}");
            return Err("Failed to get socket send buffer size");
        }
    };

    if current_size >= size {
        warn!("New buffer size is smaller than current buffer size");
        return Ok(());
    }

    // Set bigger buffer size
    let setsockopt_result = unsafe {
        libc::setsockopt(
            socket,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &size as *const _ as _,
            size_len
        )
    };

    if setsockopt_result == -1 {
        return Err("Failed to set socket send buffer size");
    }

    match get_socket_send_buffer_size(socket) {
        Ok(x) => {
            info!("New socket send buffer size: {}", x);
            Ok(())
        },
        Err(x) => {
            error!("{x}");
            Err("Failed to get new socket send buffer size")
        }
    }
}


fn get_socket_send_buffer_size(socket: i32) -> Result<u32, &'static str> {
    let mut size_len = std::mem::size_of::<u32>() as libc::socklen_t;
    let current_size: u32 = 0;

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
        Err("Failed to get socket send buffer size")
    } else {
        Ok(current_size)
    }
}

pub fn set_socket_receive_buffer_size(socket: i32, size: u32) -> Result<(), &'static str> {
    let size_len = std::mem::size_of::<u32>() as libc::socklen_t;

    let current_size = match get_socket_receive_buffer_size(socket) {
        Ok(x) => {
            info!("Current socket receive buffer size: {}", x);
            x
        },
        Err(x) => {
            error!("{x}");
            return Err("Failed to get socket receive buffer size");
        }
    };

    if current_size >= size {
        warn!("New buffer size is smaller than current buffer size");
        return Ok(());
    }

    // Set bigger buffer size
    let setsockopt_result = unsafe {
        libc::setsockopt(
            socket,
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            &size as *const _ as _,
            size_len
        )
    };

    if setsockopt_result == -1 {
        return Err("Failed to set socket receive buffer size");
    }

    match get_socket_receive_buffer_size(socket) {
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


fn get_socket_receive_buffer_size(socket: i32) -> Result<u32, &'static str> {
    let mut size_len = std::mem::size_of::<u32>() as libc::socklen_t;
    let current_size: u32 = 0;

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
        Err("Failed to get socket receive buffer size")
    } else {
        Ok(current_size)
    }
}
