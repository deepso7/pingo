use crate::error::Result;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

const STUN_SERVER: &str = "stun.l.google.com:19302";
const BINDING_REQUEST: [u8; 20] = [
    0x00, 0x01, // Message Type: Binding Request
    0x00, 0x00, // Message Length: 0 bytes
    0x21, 0x12, 0xA4, 0x42, // Magic Cookie
    // Transaction ID (96 bits / 12 bytes)
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
];

pub struct Stun {}

impl Stun {
    pub fn resolve_public_address() -> Result<(Vec<String>, Vec<String>)> {
        let server_addrs = STUN_SERVER.to_socket_addrs()?;

        // ipv4, ipv6
        let mut public_addresses = (Vec::new(), Vec::new());

        for addr in server_addrs {
            if addr.is_ipv4() {
                let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
                let socket = UdpSocket::bind(local_addr)?;

                socket.set_read_timeout(Some(Duration::from_secs(5)))?;

                debug!("local port: {}", socket.local_addr()?.port());

                match socket.send_to(&BINDING_REQUEST, addr) {
                    Ok(sent) => {
                        debug!("Sent {} bytes to server", sent);

                        // Buffer to receive response
                        let mut buf = [0u8; 512];

                        match socket.recv_from(&mut buf) {
                            Ok((size, addr)) => {
                                debug!("Received {} bytes from {}", size, addr);

                                if size < 20 {
                                    warn!("Response too short");
                                    continue;
                                }

                                // Parse XOR-MAPPED-ADDRESS
                                let mut i = 20;
                                while i < size {
                                    let attr_type = ((buf[i] as u16) << 8) | (buf[i + 1] as u16);
                                    let attr_length =
                                        ((buf[i + 2] as u16) << 8) | (buf[i + 3] as u16);

                                    // XOR-MAPPED-ADDRESS type is 0x0020
                                    if attr_type == 0x0020 {
                                        if attr_length >= 8 {
                                            // Skip attribute header and address family
                                            let port = ((buf[i + 6] as u16) << 8
                                                | (buf[i + 7] as u16))
                                                ^ 0x2112;
                                            let ip = [
                                                buf[i + 8] ^ 0x21,
                                                buf[i + 9] ^ 0x12,
                                                buf[i + 10] ^ 0xA4,
                                                buf[i + 11] ^ 0x42,
                                            ];

                                            let public_addr = format!(
                                                "{}.{}.{}.{}:{}",
                                                ip[0], ip[1], ip[2], ip[3], port
                                            );

                                            public_addresses.0.push(public_addr);
                                        }
                                    }
                                    i += 4 + attr_length as usize;
                                }
                            }
                            Err(e) => {
                                error!("error receiving data: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("error sending data: {}", e);
                    }
                }
            }

            if addr.is_ipv6() {
                let local_addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
                let socket = UdpSocket::bind(local_addr)?;
                socket.set_read_timeout(Some(Duration::from_secs(5)))?;

                match socket.send_to(&BINDING_REQUEST, addr) {
                    Ok(sent) => {
                        debug!("Sent {} bytes to server", sent);
                        let mut buf = [0u8; 512];

                        match socket.recv_from(&mut buf) {
                            Ok((size, addr)) => {
                                debug!("Received {} bytes from {}", size, addr);
                                if size < 20 {
                                    warn!("Response too short");
                                    continue;
                                }

                                let mut i = 20;
                                while i < size {
                                    let attr_type = ((buf[i] as u16) << 8) | (buf[i + 1] as u16);
                                    let attr_length =
                                        ((buf[i + 2] as u16) << 8) | (buf[i + 3] as u16);

                                    if attr_type == 0x0020 {
                                        if attr_length >= 20 {
                                            let family =
                                                ((buf[i + 4] as u16) << 8) | (buf[i + 5] as u16);
                                            if family == 0x02 {
                                                let port = ((buf[i + 6] as u16) << 8
                                                    | (buf[i + 7] as u16))
                                                    ^ 0x2112;

                                                let mut ipv6_bytes = [0u8; 16];
                                                for j in 0..16 {
                                                    if j < 4 {
                                                        ipv6_bytes[j] =
                                                            buf[i + 8 + j] ^ BINDING_REQUEST[4 + j];
                                                    } else {
                                                        ipv6_bytes[j] = buf[i + 8 + j]
                                                            ^ BINDING_REQUEST[4 + (j % 4)];
                                                    }
                                                }

                                                let ipv6 = Ipv6Addr::from(ipv6_bytes);

                                                let public_addr = format!("{}:{}", ipv6, port);

                                                public_addresses.1.push(public_addr);
                                            }
                                        }
                                    }
                                    i += 4 + attr_length as usize;
                                }
                            }
                            Err(e) => {
                                println!("Error receiving response: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error sending request: {}", e);
                    }
                }
            }
        }

        Ok(public_addresses)
    }
}
