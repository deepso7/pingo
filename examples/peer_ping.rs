//! Interactive P2P latency measurement example.
//!
//! Usage:
//!   cargo run --example peer_ping              # Interactive mode
//!   cargo run --example peer_ping -- --listen 9999   # Server mode (EC2/VPS)

use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use pingo::{get_public_addr_with_socket, measure_latency};

const PUNCH_MAGIC: &[u8] = b"PINGO_PUNCH";
const PING: u8 = 0x01;
const PONG: u8 = 0x02;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        match args[1].as_str() {
            "--help" | "-h" => {
                print_usage();
                return;
            }
            "--listen" => {
                let port = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                    eprintln!("Error: --listen requires a port number");
                    std::process::exit(1);
                });
                run_server(port);
                return;
            }
            _ => {}
        }
    }

    run_interactive();
}

fn print_usage() {
    println!("Pingo - P2P Latency Measurement");
    println!();
    println!("Usage:");
    println!("  peer_ping              Interactive P2P mode");
    println!("  peer_ping --listen PORT    Server mode (for EC2/VPS)");
    println!();
    println!("In interactive mode, both peers run the command, exchange");
    println!("addresses, and enter them simultaneously to establish a connection.");
}

/// Interactive P2P mode - discovers address and waits for peer input.
fn run_interactive() {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
    let public_addr = get_public_addr_with_socket(Some(&socket)).expect("STUN query failed");

    println!();
    println!("════════════════════════════════════════════");
    println!("  Your address: {}", public_addr);
    println!("════════════════════════════════════════════");
    println!();
    println!("Share this with your peer, then enter their address.");
    println!("(Press 'q' to quit)");
    println!();

    print!("> ");
    io::stdout().flush().ok();

    let peer_addr = match read_peer_address() {
        Some(addr) => addr,
        None => return,
    };

    println!();
    println!("Connecting to {}...", peer_addr);

    run_bidirectional(socket, peer_addr);
}

/// Reads and parses a peer address from stdin.
fn read_peer_address() -> Option<SocketAddr> {
    let stdin = io::stdin();
    let line = stdin.lock().lines().next()?;
    let input = line.ok()?;
    let input = input.trim();

    if input == "q" || input == "quit" {
        return None;
    }

    match input.parse() {
        Ok(addr) => Some(addr),
        Err(_) => {
            eprintln!("Invalid address format: {}", input);
            None
        }
    }
}

/// Server mode for EC2/VPS - listens on a fixed port.
fn run_server(port: u16) {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).expect("Failed to bind");
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();

    println!("Listening on port {}...", port);
    println!();
    println!("Peer command:");
    println!("  cargo run --example peer_ping");
    println!("  Then enter: <your-public-ip>:{}", port);
    println!();
    println!("Waiting for peer...");

    let mut buf = [0u8; 64];
    let mut peer: Option<SocketAddr> = None;
    let mut last_ping_sent = Instant::now();
    let mut ping_seq: u32 = 0;
    let mut rtts: Vec<Duration> = Vec::new();
    let mut ping_times: HashMap<u32, Instant> = HashMap::new();

    loop {
        if let Ok((len, from)) = socket.recv_from(&mut buf) {
            if peer.is_none() {
                peer = Some(from);
                println!("Peer connected: {}", from);
                println!();
            }

            handle_packet(&socket, &buf[..len], from, &mut ping_times, &mut rtts);
        }

        // Send pings to connected peer
        if let Some(peer_addr) = peer {
            if last_ping_sent.elapsed() > Duration::from_millis(200) {
                send_ping(&socket, peer_addr, ping_seq, &mut ping_times);
                ping_seq = ping_seq.wrapping_add(1);
                last_ping_sent = Instant::now();
            }
        }
    }
}

/// Handles an incoming packet (punch, ping, or pong).
fn handle_packet(
    socket: &UdpSocket,
    data: &[u8],
    from: SocketAddr,
    ping_times: &mut HashMap<u32, Instant>,
    rtts: &mut Vec<Duration>,
) {
    let len = data.len();

    // Punch packet
    if len == PUNCH_MAGIC.len() && data == PUNCH_MAGIC {
        socket.send_to(PUNCH_MAGIC, from).ok();
        return;
    }

    // Ping packet - respond with pong
    if len >= 13 && data[0] == PING {
        let mut pong = data.to_vec();
        pong[0] = PONG;
        socket.send_to(&pong, from).ok();
        return;
    }

    // Pong packet - calculate RTT
    if len >= 5 && data[0] == PONG {
        let seq = u32::from_be_bytes([data[1], data[2], data[3], data[4]]);
        if let Some(send_time) = ping_times.remove(&seq) {
            let rtt = send_time.elapsed();
            rtts.push(rtt);
            println!("  Ping {:2}: {:>10.2?}", rtts.len(), rtt);

            if rtts.len() >= 10 {
                print_stats(rtts);
                rtts.clear();
                println!();
            }
        }
    }
}

/// Sends a ping packet.
fn send_ping(
    socket: &UdpSocket,
    peer: SocketAddr,
    seq: u32,
    ping_times: &mut HashMap<u32, Instant>,
) {
    let mut ping = vec![PING];
    ping.extend_from_slice(&seq.to_be_bytes());
    ping.extend_from_slice(&[0u8; 8]);

    if socket.send_to(&ping, peer).is_ok() {
        ping_times.insert(seq, Instant::now());
        // Clean old entries
        ping_times.retain(|_, t| t.elapsed() < Duration::from_secs(5));
    }
}

/// Bidirectional connection with hole punching.
fn run_bidirectional(socket: UdpSocket, peer_addr: SocketAddr) {
    let socket_clone = socket.try_clone().expect("Failed to clone socket");
    let (tx, rx) = mpsc::channel();
    let peer_ip = peer_addr.ip();

    // Spawn responder thread
    thread::spawn(move || {
        let mut buf = [0u8; 64];
        socket_clone
            .set_read_timeout(Some(Duration::from_millis(50)))
            .ok();

        loop {
            if let Ok((len, from)) = socket_clone.recv_from(&mut buf) {
                if from.ip() != peer_ip {
                    continue;
                }

                if len == PUNCH_MAGIC.len() && buf[..len] == *PUNCH_MAGIC {
                    tx.send(from).ok();
                    socket_clone.send_to(PUNCH_MAGIC, from).ok();
                } else if len >= 13 && buf[0] == PING {
                    let mut pong = buf[..len].to_vec();
                    pong[0] = PONG;
                    socket_clone.send_to(&pong, from).ok();
                }
            }
        }
    });

    // Hole punching
    println!("Punching hole...");

    let start = Instant::now();
    let timeout = Duration::from_secs(15);
    let mut actual_peer = peer_addr;
    let mut connected = false;

    while start.elapsed() < timeout {
        socket.send_to(PUNCH_MAGIC, peer_addr).ok();

        if let Ok(from) = rx.try_recv() {
            actual_peer = from;
            connected = true;
            for _ in 0..5 {
                socket.send_to(PUNCH_MAGIC, actual_peer).ok();
                thread::sleep(Duration::from_millis(20));
            }
            break;
        }

        thread::sleep(Duration::from_millis(50));
    }

    if !connected {
        eprintln!("Connection failed!");
        eprintln!();
        eprintln!("Tips:");
        eprintln!("  - Both peers must enter addresses around the same time");
        eprintln!("  - Some NATs (symmetric) don't support hole punching");
        return;
    }

    println!("Connected to {}!", actual_peer);
    println!();
    println!("Measuring latency...");
    println!();

    match measure_latency(&socket, actual_peer, 10) {
        Ok(stats) => {
            for (i, sample) in stats.samples.iter().enumerate() {
                println!("  Ping {:2}: {:>10.2?}", i + 1, sample);
            }
            println!();
            print_stats(&stats.samples);
        }
        Err(e) => eprintln!("Measurement failed: {}", e),
    }

    println!();
    println!("Staying online for peer (30s)...");
    thread::sleep(Duration::from_secs(30));
    println!("Done.");
}

/// Prints latency statistics.
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
