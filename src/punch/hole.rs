//! UDP hole punching implementation.

use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

use crate::Result;

/// Timeout for hole punching attempts.
const PUNCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Interval between sending punch packets.
const PUNCH_INTERVAL: Duration = Duration::from_millis(100);

/// Magic bytes to identify punch packets.
const PUNCH_MAGIC: &[u8] = b"PINGO_PUNCH";

/// Punches a UDP hole to the peer's public address.
///
/// Both peers must call this simultaneously. Each side sends packets to the other's
/// public address, causing their NATs to create mappings. When the packets cross,
/// the hole is punched and bidirectional communication becomes possible.
///
/// # Arguments
///
/// * `socket` - The UDP socket to use (should be the same one used for STUN)
/// * `peer_addr` - The peer's public address (obtained via STUN)
///
/// # Returns
///
/// `Ok(())` if the hole was punched successfully, `Err` if it timed out.
pub fn punch_hole(socket: &UdpSocket, peer_addr: SocketAddr) -> Result<()> {
    socket.set_read_timeout(Some(PUNCH_INTERVAL))?;
    socket.set_nonblocking(false)?;

    let start = Instant::now();

    while start.elapsed() < PUNCH_TIMEOUT {
        // Send punch packet
        socket.send_to(PUNCH_MAGIC, peer_addr)?;

        // Try to receive response
        let mut buf = [0u8; 64];
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                // Check if it's a valid punch response from the peer's IP
                // (port might differ due to symmetric NAT)
                if from.ip() == peer_addr.ip()
                    && len >= PUNCH_MAGIC.len()
                    && buf[..PUNCH_MAGIC.len()] == *PUNCH_MAGIC
                {
                    // Send a few more packets to ensure peer receives them
                    for _ in 0..3 {
                        socket.send_to(PUNCH_MAGIC, peer_addr)?;
                        thread::sleep(Duration::from_millis(50));
                    }
                    return Ok(());
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }

    Err("hole punch timeout - no response from peer".into())
}
