use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use pingo::{get_public_addr_with_socket, measure_latency};

const PUNCH_MAGIC: &[u8] = b"PINGO_PUNCH";
const PING: u8 = 0x01;
const PONG: u8 = 0x02;

fn print_usage() {
    eprintln!("Usage: cargo run --example peer_ping -- <peer_ip:port>");
    eprintln!();
    eprintln!("Bidirectional P2P latency measurement.");
    eprintln!();
    eprintln!("Setup:");
    eprintln!("  1. Run on both machines to get public addresses:");
    eprintln!("     cargo run --example peer_ping");
    eprintln!();
    eprintln!("  2. Exchange addresses, then both run simultaneously:");
    eprintln!("     cargo run --example peer_ping -- <peer's_address>");
    eprintln!();
    eprintln!("For EC2/VPS (no NAT), use --listen mode:");
    eprintln!("     cargo run --example peer_ping -- --listen 9999");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        // No args - just show our public address
        show_public_addr();
        return;
    }

    match args[1].as_str() {
        "--help" | "-h" => print_usage(),
        "--listen" => {
            let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                eprintln!("Error: --listen requires a port number");
                std::process::exit(1);
            });
            run_server(port);
        }
        addr => {
            let peer_addr: SocketAddr = addr.parse().unwrap_or_else(|_| {
                eprintln!("Invalid address: {}", addr);
                print_usage();
                std::process::exit(1);
            });
            run_bidirectional(peer_addr);
        }
    }
}

fn show_public_addr() {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind");
    let public_addr = get_public_addr_with_socket(Some(&socket)).expect("STUN failed");

    println!("Your public address: {}", public_addr);
    println!();
    println!("Share this with your peer, then both run:");
    println!("  cargo run --example peer_ping -- <peer's_address>");
    println!();
    println!("Both must start within a few seconds of each other!");
}

/// Server mode for EC2/VPS - responds to pings and also pings back
fn run_server(port: u16) {
    println!("Listening on port {}...", port);
    println!(
        "Peer should run: cargo run --example peer_ping -- <your-ip>:{}",
        port
    );
    println!();

    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).expect("Failed to bind");
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();

    let mut buf = [0u8; 64];
    let mut peer: Option<SocketAddr> = None;
    let mut last_ping_sent = Instant::now();
    let mut ping_seq: u32 = 0;
    let mut rtts: Vec<Duration> = Vec::new();

    println!("Waiting for peer...");

    loop {
        // Receive
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                if peer.is_none() {
                    peer = Some(from);
                    println!("Peer connected: {}", from);
                    println!();
                }

                // Respond to punch
                if len == PUNCH_MAGIC.len() && &buf[..len] == PUNCH_MAGIC {
                    socket.send_to(PUNCH_MAGIC, from).ok();
                }
                // Respond to ping with pong
                else if len >= 13 && buf[0] == PING {
                    let mut pong = buf[..len].to_vec();
                    pong[0] = PONG;
                    socket.send_to(&pong, from).ok();
                }
                // Process pong (response to our ping)
                else if len >= 13 && buf[0] == PONG {
                    let recv_seq = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
                    if recv_seq == ping_seq.wrapping_sub(1) {
                        let rtt = last_ping_sent.elapsed();
                        rtts.push(rtt);
                        println!("  Ping {:2}: {:>10.2?}", rtts.len(), rtt);

                        if rtts.len() >= 10 {
                            print_stats(&rtts);
                            rtts.clear();
                        }
                    }
                }
            }
            Err(_) => {}
        }

        // Send our own pings to peer
        if let Some(peer_addr) = peer {
            if last_ping_sent.elapsed() > Duration::from_millis(200) {
                let mut ping = vec![PING];
                ping.extend_from_slice(&ping_seq.to_be_bytes());
                ping.extend_from_slice(&[0u8; 8]); // timestamp placeholder
                socket.send_to(&ping, peer_addr).ok();
                ping_seq = ping_seq.wrapping_add(1);
                last_ping_sent = Instant::now();
            }
        }
    }
}

/// Bidirectional mode - both sides punch and ping simultaneously
fn run_bidirectional(peer_addr: SocketAddr) {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind");

    // Get public address using same socket (preserves NAT mapping)
    let public_addr = get_public_addr_with_socket(Some(&socket)).expect("STUN failed");
    println!("Your public address: {}", public_addr);
    println!("Peer address: {}", peer_addr);
    println!();

    // Clone socket for the responder thread
    let socket_clone = socket.try_clone().expect("Failed to clone socket");

    // Channel to signal when hole is punched
    let (tx, rx) = mpsc::channel();

    // Responder thread - responds to pings from peer
    let peer_addr_clone = peer_addr;
    thread::spawn(move || {
        let mut buf = [0u8; 64];
        socket_clone
            .set_read_timeout(Some(Duration::from_millis(50)))
            .ok();

        loop {
            match socket_clone.recv_from(&mut buf) {
                Ok((len, from)) => {
                    if from.ip() != peer_addr_clone.ip() {
                        continue;
                    }

                    // Punch packet - signal main thread
                    if len == PUNCH_MAGIC.len() && &buf[..len] == PUNCH_MAGIC {
                        tx.send(()).ok();
                        socket_clone.send_to(PUNCH_MAGIC, from).ok();
                    }
                    // Ping - respond with pong
                    else if len >= 13 && buf[0] == PING {
                        let mut pong = buf[..len].to_vec();
                        pong[0] = PONG;
                        socket_clone.send_to(&pong, from).ok();
                    }
                }
                Err(_) => {}
            }
        }
    });

    // Punch hole
    println!("Punching hole...");
    let start = Instant::now();
    let timeout = Duration::from_secs(10);
    let mut connected = false;

    while start.elapsed() < timeout {
        socket.send_to(PUNCH_MAGIC, peer_addr).ok();

        // Check if responder thread received something
        if rx.try_recv().is_ok() {
            connected = true;
            // Send a few more to ensure peer receives
            for _ in 0..5 {
                socket.send_to(PUNCH_MAGIC, peer_addr).ok();
                thread::sleep(Duration::from_millis(20));
            }
            break;
        }

        thread::sleep(Duration::from_millis(50));
    }

    if !connected {
        eprintln!("Failed to connect. Make sure both peers start at the same time.");
        return;
    }

    println!("Connected!");
    println!();

    // Measure latency
    println!("Measuring latency...");
    println!();

    match measure_latency(&socket, peer_addr, 10) {
        Ok(stats) => {
            for (i, sample) in stats.samples.iter().enumerate() {
                println!("  Ping {:2}: {:>10.2?}", i + 1, sample);
            }
            println!();
            print_stats(&stats.samples);
        }
        Err(e) => {
            eprintln!("Failed: {}", e);
        }
    }

    // Keep responding for a bit so peer can measure us too
    println!();
    println!("Staying online for peer to measure back (30s)...");
    thread::sleep(Duration::from_secs(30));
}

fn print_stats(samples: &[Duration]) {
    if samples.is_empty() {
        return;
    }
    let min = samples.iter().min().unwrap();
    let max = samples.iter().max().unwrap();
    let sum: Duration = samples.iter().sum();
    let avg = sum / samples.len() as u32;

    println!("  ─────────────────────");
    println!("  Min: {:>10.2?}", min);
    println!("  Avg: {:>10.2?}", avg);
    println!("  Max: {:>10.2?}", max);
}
