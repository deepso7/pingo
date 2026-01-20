use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::Result;

/// Hole punch timeout
const PUNCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Interval between punch attempts
const PUNCH_INTERVAL: Duration = Duration::from_millis(100);

/// Magic bytes to identify punch packets
const PUNCH_MAGIC: &[u8] = b"PINGO_PUNCH";

/// Punch a hole to the peer's public address
///
/// Both peers must call this simultaneously, sending packets to each other's
/// public address. The NAT will create a mapping, and when packets cross,
/// the hole is punched.
pub fn punch_hole(socket: &UdpSocket, peer_addr: SocketAddr) -> Result<()> {
    socket.set_read_timeout(Some(PUNCH_INTERVAL))?;
    socket.set_nonblocking(false)?;

    let start = Instant::now();
    let mut received_from_peer = false;
    let mut send_count = 0;

    eprintln!("[punch] Starting hole punch to {}", peer_addr);

    while start.elapsed() < PUNCH_TIMEOUT {
        // Send punch packet
        socket.send_to(PUNCH_MAGIC, peer_addr)?;
        send_count += 1;
        if send_count % 20 == 0 {
            eprintln!(
                "[punch] Sent {} packets, waiting for response...",
                send_count
            );
        }

        // Try to receive
        let mut buf = [0u8; 64];
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                eprintln!(
                    "[punch] Received {} bytes from {} (expecting from {}): {:?}",
                    len,
                    from,
                    peer_addr,
                    &buf[..len.min(20)]
                );

                // Accept response from peer (check IP matches, port might differ due to NAT)
                if from.ip() == peer_addr.ip()
                    && len >= PUNCH_MAGIC.len()
                    && &buf[..PUNCH_MAGIC.len()] == PUNCH_MAGIC
                {
                    eprintln!("[punch] Valid punch response from peer!");
                    received_from_peer = true;
                    // Send a few more to make sure peer receives
                    for _ in 0..3 {
                        socket.send_to(PUNCH_MAGIC, peer_addr)?;
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }

    eprintln!("[punch] Finished. Sent {} packets total.", send_count);

    if received_from_peer {
        Ok(())
    } else {
        Err("hole punch timeout - no response from peer".into())
    }
}
