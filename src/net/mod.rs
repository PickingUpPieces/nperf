use std::net::Ipv4Addr;
use std::str::FromStr;

pub mod socket;
mod socket_options;

pub fn parse_ipv4(adress: String) -> Result<Ipv4Addr, &'static str> {
    match Ipv4Addr::from_str(adress.as_str()) {
        Ok(x) => Ok(x),
        Err(_) => Err("Invalid IPv4 address!"),
    }
}