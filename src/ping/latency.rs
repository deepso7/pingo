use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::Result;

/// Ping packet type
const PING: u8 = 0x01;
/// Pong packet type  
const PONG: u8 = 0x02;

/// Timeout for individual ping
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub samples: Vec<Duration>,
}

impl LatencyStats {
    fn from_samples(samples: Vec<Duration>) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }

        let min = *samples.iter().min().unwrap();
        let max = *samples.iter().max().unwrap();
        let sum: Duration = samples.iter().sum();
        let avg = sum / samples.len() as u32;

        Some(LatencyStats {
            min,
            max,
            avg,
            samples,
        })
    }
}

/// Measure latency to a peer
///
/// Sends `count` ping packets and measures round-trip time.
/// The peer must be running `respond_to_pings` for this to work.
pub fn measure_latency(
    socket: &UdpSocket,
    peer_addr: SocketAddr,
    count: u32,
) -> Result<LatencyStats> {
    // Drain any leftover packets in the buffer
    socket.set_read_timeout(Some(Duration::from_millis(10)))?;
    let mut buf = [0u8; 64];
    while socket.recv_from(&mut buf).is_ok() {}

    socket.set_read_timeout(Some(PING_TIMEOUT))?;

    let mut samples = Vec::with_capacity(count as usize);

    for i in 0..count {
        // Create ping packet with sequence number and timestamp
        let seq = i as u32;
        let send_time = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let mut ping_packet = vec![PING];
        ping_packet.extend_from_slice(&seq.to_be_bytes());
        ping_packet.extend_from_slice(&timestamp.to_be_bytes());

        socket.send_to(&ping_packet, peer_addr)?;
        eprintln!("[ping] Sent ping seq={} to {}", seq, peer_addr);

        // Wait for matching pong
        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, from)) => {
                    eprintln!(
                        "[ping] Received {} bytes from {}: type=0x{:02x}, data={:?}",
                        len,
                        from,
                        buf[0],
                        &buf[..len.min(16)]
                    );

                    // Check it's a valid pong with matching sequence
                    if from.ip() == peer_addr.ip() && len >= 13 && buf[0] == PONG {
                        let recv_seq = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
                        if recv_seq == seq {
                            let rtt = send_time.elapsed();
                            eprintln!("[ping] Valid pong for seq={}, RTT={:?}", seq, rtt);
                            samples.push(rtt);
                            break;
                        } else {
                            eprintln!(
                                "[ping] Sequence mismatch: expected {}, got {}",
                                seq, recv_seq
                            );
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    eprintln!("[ping] Timeout waiting for pong seq={}", seq);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    eprintln!("[ping] Timeout waiting for pong seq={}", seq);
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Small delay between pings
        std::thread::sleep(Duration::from_millis(100));
    }

    LatencyStats::from_samples(samples).ok_or_else(|| "no successful pings".into())
}

/// Respond to incoming pings
///
/// Call this in a loop to respond to pings from peers.
/// Returns the address of the peer that sent the ping.
pub fn respond_to_ping(socket: &UdpSocket) -> Result<Option<SocketAddr>> {
    let mut buf = [0u8; 64];

    match socket.recv_from(&mut buf) {
        Ok((len, from)) => {
            if len >= 13 && buf[0] == PING {
                // Echo back as pong
                let mut pong_packet = buf[..len].to_vec();
                pong_packet[0] = PONG;
                socket.send_to(&pong_packet, from)?;
                return Ok(Some(from));
            }
            Ok(None)
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(None),
        Err(e) => Err(e.into()),
    }
}
