use log::{error, info, debug, warn};

#[derive(PartialEq, Debug)]
pub struct SocketOptions {
    nonblocking: bool,
    without_ip_frag: bool,
    gso: (bool, u32),
    _gro: (bool, u32),
    recv_buffer_size: u32,
    send_buffer_size: u32,
}

impl SocketOptions {
    pub fn new(nonblocking: bool, without_ip_frag: bool, gso: (bool, u32), _gro: (bool, u32), recv_buffer_size: u32, send_buffer_size: u32) -> Self {
        SocketOptions {
            nonblocking,
            without_ip_frag,
            gso,
            _gro,
            recv_buffer_size,
            send_buffer_size,
        }
    }

    pub fn update(&mut self, socket: i32) -> Result<(), &'static str> {
        debug!("Updating socket options with {:?}", self);
        if self.nonblocking {
            self.set_nonblocking(socket)?;
        } 
        if self.without_ip_frag {
            self.set_without_ip_frag(socket)?;
        } 
        if self.gso.0 {
            self.set_gso(socket, self.gso.1)?;
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
            error!("errno when enabling socket option on socket: {}", unsafe { *libc::__errno_location() });
            return Err("Failed to enable socket option");
        }

        debug!("Enabled socket option on socket with value {}", value);
        Ok(())
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
        debug!("Trying to set send buffer size from {} to {}", current_size, size);
    
        if current_size >= size {
            warn!("New buffer size {} is smaller than current buffer size {}", size, current_size);
            return Ok(());
        }

        match Self::set_socket_option(socket, libc::SOL_SOCKET, libc::SO_SNDBUF, size) {
            Ok(_) => {},
            Err(x) => {
                error!("{x}");
                return Err("Failed to set socket send buffer size");
            }
        }
    
        // TODO: Check only for okay with if let
        match Self::get_send_buffer_size(socket) {
            Ok(x) => {
                if x == size {
                    info!("New socket send buffer size: {}", x);
                    self.send_buffer_size = x;
                    Ok(())
                } else {
                    error!("Current send buffer size not equal desired one: {} vs {}", x, size);
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
        let current_size = Self::get_receive_buffer_size(socket)?; 
        debug!("Trying to set receive buffer size from {} to {}", current_size, size);
    
        if current_size >= size {
            warn!("New buffer size {} is smaller than current buffer size {}", size, current_size);
            return Ok(());
        }

        match Self::set_socket_option(socket, libc::SOL_SOCKET, libc::SO_RCVBUF, size) {
            Ok(_) => {},
            Err(x) => {
                error!("{x}");
                return Err("Failed to set socket receive buffer size");
            }
        }
    
        match Self::get_receive_buffer_size(socket) {
            Ok(x) => {
                if x == size {
                    info!("New socket receive buffer size: {}", x);
                    self.recv_buffer_size = x;
                    Ok(())
                } else {
                    error!("Current receive buffer size not equal desired one: {} vs {}", x, size);
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

    pub fn set_gso(&mut self, socket: i32, gso_size: u32) -> Result<(), &'static str> {
        // gso_size should be equal to MTU = ETH_MTU - header(ipv4) - header(udp)
        info!("Trying to set socket option GSO to {}", gso_size);
        Self::set_socket_option(socket, libc::SOL_UDP, libc::UDP_SEGMENT, gso_size)
    }

    pub fn set_without_ip_frag(&mut self, socket: i32) -> Result<(), &'static str> {
        let value: u32 = 1;
        info!("Trying to set socket option IP_DONTFRAG to {}", value);

        // Normally the option should be IP_DONTFRAG, but this fails to resolve
        // Self::set_socket_option(socket, libc::IPPROTO_IP, libc::IP_DONTFRAG, value)
        Self::set_socket_option(socket, libc::IPPROTO_IP, libc::IP_MTU_DISCOVER, libc::IP_PMTUDISC_DO.try_into().unwrap())
    }

    pub fn get_mtu(&self, socket: i32) -> Result<u32, &'static str> {
        // https://man7.org/linux/man-pages/man7/ip.7.html
        // MSS from TCP returned an error
        let current_size: u32 = 0;
        let mut size_len = std::mem::size_of_val(&current_size) as libc::socklen_t;

        // IP_MTU
        let getsockopt_result = unsafe {
            libc::getsockopt(
                socket,
                libc::IPPROTO_IP,
                libc::IP_MTU,
                &current_size as *const _ as _,
                &mut size_len as *mut _
            )
        };

        if getsockopt_result == -1 {
            error!("errno when getting Ethernet MTU: {}", unsafe { *libc::__errno_location() });
            Err("Failed to get socket Ethernet MTU")
        } else {
            info!("Current socket Ethernet MTU: {}", current_size);
            // Minus IP header size and UDP header size
            Ok(current_size - 20 - 8)
        }
    }
}