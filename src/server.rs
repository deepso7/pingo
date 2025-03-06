use std::{
    io::ErrorKind,
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use crate::{constant::PACKET_SIZE, error::Result};

pub struct Server {}

impl Server {
    pub fn init(target_addr: SocketAddr, local_port: usize) -> Result<()> {
        let local_addr = format!("0.0.0.0:{}", local_port).parse::<SocketAddr>()?;
        let socket = UdpSocket::bind(local_addr)?;

        debug!("Starting UDP ping server on {}", socket.local_addr()?);
        debug!("Target address: {}", target_addr);

        socket.set_read_timeout(Some(Duration::from_secs(1)))?;

        // Initialize buffer for receiving data
        let mut buf = [0u8; PACKET_SIZE];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, src)) => {
                    debug!("Received {} bytes from {}", size, src);

                    // Send the same data back
                    match socket.send_to(&buf[0..size], src) {
                        Ok(_) => {}
                        Err(e) => error!("Failed to send response: {}", e),
                    }

                    // Also try to send to the target address
                    // This helps with NAT hole punching
                    if src != target_addr {
                        match socket.send_to(&buf[0..size], target_addr) {
                            Ok(_) => debug!("Sent hole-punching packet to target"),
                            Err(e) => error!("Failed to send hole-punching packet: {}", e),
                        }
                    }
                }
                Err(e) => {
                    if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock {
                        // Send periodic hole-punching packets
                        let punch_data = b"PUNCH";
                        match socket.send_to(punch_data, target_addr) {
                            Ok(_) => {}
                            Err(e) => error!("Failed to send hole-punching packet: {}", e),
                        }
                    } else {
                        error!("Error receiving data: {}", e);
                    }
                }
            }
        }
    }
}
