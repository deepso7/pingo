use std::{
    io::{Error, ErrorKind},
    net::{SocketAddr, UdpSocket},
    thread,
    time::{Duration, Instant},
};

use crate::{
    constant::{DEFAULT_COUNT, PACKET_SIZE},
    error::Result,
};

pub struct Client {}

impl Client {
    pub fn init(target_addr: SocketAddr, local_port: usize) -> Result<()> {
        let local_addr = format!("0.0.0.0:{}", local_port)
            .parse::<SocketAddr>()
            .unwrap();
        let socket = UdpSocket::bind(local_addr)?;

        debug!("Starting UDP ping client from {}", socket.local_addr()?);
        debug!("Target address: {}", target_addr);

        // Set socket options
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;

        // Send initial hole punching packets
        debug!("Sending initial hole punching packets...");
        for _ in 0..5 {
            let punch_data = b"PUNCH";
            socket.send_to(punch_data, target_addr)?;
            thread::sleep(Duration::from_millis(200));
        }

        // Number of pings to send
        let count = DEFAULT_COUNT;

        // Collect latency statistics
        let mut received = 0;
        let mut latencies = Vec::with_capacity(count);

        for i in 1..=count {
            match measure_latency(&socket, target_addr, i) {
                Ok(latency) => {
                    debug!("Ping {}: {} ms", i, latency.as_millis());
                    latencies.push(latency);
                    received += 1;
                }
                Err(e) => {
                    error!("Ping {} failed: {}", i, e);
                }
            }

            // Wait between pings
            thread::sleep(Duration::from_secs(1));
        }

        // Calculate statistics
        if !latencies.is_empty() {
            let avg =
                latencies.iter().sum::<Duration>().as_millis() as f64 / latencies.len() as f64;

            // Find min and max
            let min = latencies.iter().min().unwrap().as_millis();
            let max = latencies.iter().max().unwrap().as_millis();

            debug!("\nStatistics:");
            debug!(
                "  Packets: Sent = {}, Received = {}, Lost = {} ({}% loss)",
                count,
                received,
                count - received,
                (count - received) as f64 * 100.0 / count as f64
            );
            debug!(
                "  Round-trip (ms): Min = {}, Max = {}, Avg = {:.2}",
                min, max, avg
            );
        }

        Ok(())
    }
}

// Measure the round-trip time to the target address
fn measure_latency(socket: &UdpSocket, target_addr: SocketAddr, seq: usize) -> Result<Duration> {
    // Create ping packet with sequence number and timestamp
    let mut packet = [0u8; PACKET_SIZE];

    // Add sequence number (first 4 bytes)
    packet[0..4].copy_from_slice(&(seq as u32).to_le_bytes());

    // Add current timestamp (next 8 bytes)
    let now = Instant::now();
    let timestamp = now.elapsed().as_nanos() as u64;
    packet[4..12].copy_from_slice(&timestamp.to_le_bytes());

    // Fill the rest with a pattern
    for i in 12..PACKET_SIZE {
        packet[i] = (i % 256) as u8;
    }

    // Send the packet
    socket.send_to(&packet, target_addr)?;

    // Wait for response
    let mut response = [0u8; PACKET_SIZE];
    let (bytes_received, src) = socket.recv_from(&mut response)?;

    if bytes_received == 0 {
        return Err(Box::new(Error::new(ErrorKind::Other, "Empty response")));
    }

    // Verify the source address
    if src != target_addr {
        debug!("Warning: Response from unexpected source: {}", src);
    }

    // Verify the sequence number
    let received_seq = u32::from_le_bytes([response[0], response[1], response[2], response[3]]);
    if received_seq != seq as u32 {
        return Err(Box::new(Error::new(
            ErrorKind::Other,
            format!("Sequence mismatch: expected {}, got {}", seq, received_seq),
        )));
    }

    // Calculate round-trip time
    let latency = now.elapsed();

    Ok(latency)
}
