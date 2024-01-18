use log::{error, info, debug, warn};


pub struct SocketOptions {
    nonblocking: bool,
    gso: bool,
    gro: bool,
    recv_buffer_size: u32,
    send_buffer_size: u32,
}

impl SocketOptions {
    pub fn new(nonblocking: bool, gso: bool, gro: bool, recv_buffer_size: u32, send_buffer_size: u32) -> Self {
        SocketOptions {
            nonblocking,
            gso,
            gro,
            recv_buffer_size,
            send_buffer_size,
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

        self.nonblocking = true;
        Ok(())
    }

    pub fn set_send_buffer_size(&mut self, socket: i32, size: u32) -> Result<(), &'static str> {
        let size_len = std::mem::size_of_val(&size) as libc::socklen_t;
        let current_size = Self::get_send_buffer_size(socket)?;
        debug!("Trying to set send buffer size from {} to {}", current_size, size);
    
        if current_size >= size {
            warn!("New buffer size {} is smaller than current buffer size {}", size, current_size);
            return Ok(());
        }
    
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
            error!("errno when setting send buffer size: {}", unsafe { *libc::__errno_location() });
            return Err("Failed to set socket send buffer size");
        }
    
        // TODO: Check only for okay with if let
        match Self::get_send_buffer_size(socket) {
            Ok(x) => {
                if x == size {
                    info!("New socket send buffer size: {}", x);
                    self.send_buffer_size = x;
                    Ok(())
                } else {
                    error!("Current buffer size not equal desired one: {} vs {}", x, size);
                    // FIXME: Currently the max buffer size is set, not the desired one. Since this size is a lot bigger than the desired one, we fix this bug later.
                    // Err("Failed to set socket send buffer size")
                    Ok(())
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
    
    pub fn set_receive_buffer_size(&mut self, socket: i32, size: u32) -> Result<(), &'static str> {
        let size_len = std::mem::size_of_val(&size) as libc::socklen_t;
        let current_size = Self::get_receive_buffer_size(socket)?; 
        debug!("Trying to set receive buffer size from {} to {}", current_size, size);
    
        if current_size >= size {
            warn!("New buffer size {} is smaller than current buffer size {}", size, current_size);
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
            error!("errno when setting receive buffer size: {}", unsafe { *libc::__errno_location() });
            return Err("Failed to set socket receive buffer size");
        }
    
        match Self::get_receive_buffer_size(socket) {
            Ok(x) => {
                if x == size {
                    info!("New socket receive buffer size: {}", x);
                    self.recv_buffer_size = x;
                    Ok(())
                } else {
                    error!("Current buffer size not equal desired one: {} vs {}", x, size);
                    // FIXME: Currently the max buffer size is set, not the desired one. Since this size is a lot bigger than the desired one, we fix this bug later.
                    // Err("Failed to set socket receive buffer size")
                    Ok(())
                }
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

    pub fn set_gso(&mut self, socket: i32, gso_size: u64) -> Result<(), &'static str> {
        // gso_size should be equal to MTU = ETH_MTU - header(ipv4) - heaser(udp)
        let size_len = std::mem::size_of_val(&gso_size) as libc::socklen_t;

        // getsockopt(fd, SOL_UDP, UDP_SEGMENT, &gso_size, sizeof(gso_size))
        let setsockopt_result = unsafe {
            libc::setsockopt(
                socket,
                libc::SOL_UDP,
                libc::UDP_SEGMENT,
                &gso_size as *const _ as _,
                size_len
            )};

        if setsockopt_result == -1 {
            error!("errno when enabling GSO on socket: {}", unsafe { *libc::__errno_location() });
            return Err("Failed to enable GSO on socket");
        }

        self.gso = true;
        Ok(())
    }
}