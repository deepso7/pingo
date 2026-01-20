//! Latency measurement implementation.

use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::Result;

/// Ping packet type marker.
const PING: u8 = 0x01;

/// Pong packet type marker.
const PONG: u8 = 0x02;

/// Timeout for waiting for a pong response.
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Latency measurement statistics.
#[derive(Debug, Clone)]
pub struct LatencyStats {
    /// Minimum round-trip time.
    pub min: Duration,
    /// Maximum round-trip time.
    pub max: Duration,
    /// Average round-trip time.
    pub avg: Duration,
    /// All individual RTT samples.
    pub samples: Vec<Duration>,
}

impl LatencyStats {
    /// Creates stats from a list of RTT samples.
    fn from_samples(samples: Vec<Duration>) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }

        let min = *samples.iter().min().unwrap();
        let max = *samples.iter().max().unwrap();
        let sum: Duration = samples.iter().sum();
        let avg = sum / samples.len() as u32;

        Some(Self {
            min,
            max,
            avg,
            samples,
        })
    }
}

/// Measures round-trip latency to a peer.
///
/// Sends `count` ping packets and waits for pong responses, measuring the RTT.
/// The peer must be responding to pings (either via `respond_to_ping` or equivalent).
///
/// # Arguments
///
/// * `socket` - The UDP socket to use
/// * `peer_addr` - The peer's address
/// * `count` - Number of ping packets to send
///
/// # Returns
///
/// Statistics about the measured latencies.
pub fn measure_latency(
    socket: &UdpSocket,
    peer_addr: SocketAddr,
    count: u32,
) -> Result<LatencyStats> {
    // Drain any stale packets in the receive buffer
    socket.set_read_timeout(Some(Duration::from_millis(10)))?;
    let mut buf = [0u8; 64];
    while socket.recv_from(&mut buf).is_ok() {}

    socket.set_read_timeout(Some(PING_TIMEOUT))?;

    let mut samples = Vec::with_capacity(count as usize);

    for seq in 0..count {
        let send_time = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        // Build ping packet: [type:1][seq:4][timestamp:8]
        let mut ping = Vec::with_capacity(13);
        ping.push(PING);
        ping.extend_from_slice(&seq.to_be_bytes());
        ping.extend_from_slice(&timestamp.to_be_bytes());

        socket.send_to(&ping, peer_addr)?;

        // Wait for matching pong
        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, from)) => {
                    // Validate: correct peer, correct type, correct sequence
                    if from.ip() == peer_addr.ip() && len >= 5 && buf[0] == PONG {
                        let recv_seq = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
                        if recv_seq == seq {
                            samples.push(send_time.elapsed());
                            break;
                        }
                    }
                    // Ignore non-matching packets
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                    break; // Timeout, move to next ping
                }
                Err(e) => return Err(e.into()),
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    LatencyStats::from_samples(samples).ok_or_else(|| "no successful pings".into())
}

/// Responds to a single incoming ping packet.
///
/// Call this in a loop to act as a ping responder. Returns the peer's address
/// if a ping was received and responded to.
///
/// # Returns
///
/// `Ok(Some(addr))` if a ping was received from `addr` and a pong was sent,
/// `Ok(None)` if no ping was received (timeout), or `Err` on socket error.
pub fn respond_to_ping(socket: &UdpSocket) -> Result<Option<SocketAddr>> {
    let mut buf = [0u8; 64];

    match socket.recv_from(&mut buf) {
        Ok((len, from)) => {
            if len >= 13 && buf[0] == PING {
                // Echo back as pong
                let mut pong = buf[..len].to_vec();
                pong[0] = PONG;
                socket.send_to(&pong, from)?;
                return Ok(Some(from));
            }
            Ok(None)
        }
        Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => Ok(None),
        Err(e) => Err(e.into()),
    }
}
