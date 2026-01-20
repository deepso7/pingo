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

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  cargo run --example peer_ping              Interactive P2P mode");
    eprintln!("  cargo run --example peer_ping -- --listen <port>   Server mode (EC2/VPS)");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        match args[1].as_str() {
            "--help" | "-h" => {
                print_usage();
                return;
            }
            "--listen" => {
                let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                    eprintln!("Error: --listen requires a port number");
                    std::process::exit(1);
                });
                run_server(port);
                return;
            }
            _ => {}
        }
    }

    // Default: interactive P2P mode
    run_interactive();
}

/// Interactive mode - get address, wait for peer input, then connect
fn run_interactive() {
    // Bind socket FIRST
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");

    // Get public address using this socket
    let public_addr = get_public_addr_with_socket(Some(&socket)).expect("STUN failed");

    println!("════════════════════════════════════════════");
    println!("  Your address: {}", public_addr);
    println!("════════════════════════════════════════════");
    println!();
    println!("Share this address with your peer.");
    println!("Enter peer's address when ready (or 'q' to quit):");
    println!();

    // Read peer address from stdin
    print!("> ");
    io::stdout().flush().ok();

    let stdin = io::stdin();
    let line = stdin.lock().lines().next();

    let peer_addr: SocketAddr = match line {
        Some(Ok(input)) => {
            let input = input.trim();
            if input == "q" || input == "quit" {
                return;
            }
            match input.parse() {
                Ok(addr) => addr,
                Err(_) => {
                    eprintln!("Invalid address: {}", input);
                    return;
                }
            }
        }
        _ => {
            eprintln!("Failed to read input");
            return;
        }
    };

    println!();
    println!("Connecting to {}...", peer_addr);

    // Run bidirectional ping
    run_bidirectional(socket, peer_addr);
}

/// Server mode for EC2/VPS - responds to pings and pings back
fn run_server(port: u16) {
    println!("Listening on port {}...", port);
    println!("Peer should run: cargo run --example peer_ping");
    println!("Then enter: <your-public-ip>:{}", port);
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
    let mut ping_send_times: std::collections::HashMap<u32, Instant> =
        std::collections::HashMap::new();

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

                // Punch packet
                if len == PUNCH_MAGIC.len() && &buf[..len] == PUNCH_MAGIC {
                    socket.send_to(PUNCH_MAGIC, from).ok();
                }
                // Ping - respond with pong
                else if len >= 13 && buf[0] == PING {
                    let mut pong = buf[..len].to_vec();
                    pong[0] = PONG;
                    socket.send_to(&pong, from).ok();
                }
                // Pong - calculate RTT
                else if len >= 5 && buf[0] == PONG {
                    let recv_seq = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
                    if let Some(send_time) = ping_send_times.remove(&recv_seq) {
                        let rtt = send_time.elapsed();
                        rtts.push(rtt);
                        println!("  Ping {:2}: {:>10.2?}", rtts.len(), rtt);

                        if rtts.len() >= 10 {
                            print_stats(&rtts);
                            rtts.clear();
                            println!();
                            println!("Continuing to measure...");
                            println!();
                        }
                    }
                }
            }
            Err(_) => {}
        }

        // Send pings to peer
        if let Some(peer_addr) = peer {
            if last_ping_sent.elapsed() > Duration::from_millis(200) {
                let mut ping = vec![PING];
                ping.extend_from_slice(&ping_seq.to_be_bytes());
                ping.extend_from_slice(&[0u8; 8]);
                socket.send_to(&ping, peer_addr).ok();
                ping_send_times.insert(ping_seq, Instant::now());
                ping_seq = ping_seq.wrapping_add(1);
                last_ping_sent = Instant::now();

                // Clean old entries
                ping_send_times.retain(|_, t| t.elapsed() < Duration::from_secs(5));
            }
        }
    }
}

/// Bidirectional ping with existing socket
fn run_bidirectional(socket: UdpSocket, peer_addr: SocketAddr) {
    // Clone socket for responder thread
    let socket_clone = socket.try_clone().expect("Failed to clone socket");

    // Channel to signal connection
    let (tx, rx) = mpsc::channel();

    // Responder thread
    let peer_ip = peer_addr.ip();
    thread::spawn(move || {
        let mut buf = [0u8; 64];
        socket_clone
            .set_read_timeout(Some(Duration::from_millis(50)))
            .ok();

        loop {
            match socket_clone.recv_from(&mut buf) {
                Ok((len, from)) => {
                    if from.ip() != peer_ip {
                        continue;
                    }

                    if len == PUNCH_MAGIC.len() && &buf[..len] == PUNCH_MAGIC {
                        tx.send(from).ok();
                        socket_clone.send_to(PUNCH_MAGIC, from).ok();
                    } else if len >= 13 && buf[0] == PING {
                        let mut pong = buf[..len].to_vec();
                        pong[0] = PONG;
                        socket_clone.send_to(&pong, from).ok();
                    }
                }
                Err(_) => {}
            }
        }
    });

    // Hole punching
    println!("Punching hole...");
    let start = Instant::now();
    let timeout = Duration::from_secs(15);
    let mut connected = false;
    let mut actual_peer_addr = peer_addr;

    while start.elapsed() < timeout {
        socket.send_to(PUNCH_MAGIC, peer_addr).ok();

        if let Ok(from) = rx.try_recv() {
            actual_peer_addr = from;
            connected = true;
            for _ in 0..5 {
                socket.send_to(PUNCH_MAGIC, actual_peer_addr).ok();
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

    println!("Connected to {}!", actual_peer_addr);
    println!();

    // Measure latency
    println!("Measuring latency...");
    println!();

    match measure_latency(&socket, actual_peer_addr, 10) {
        Ok(stats) => {
            for (i, sample) in stats.samples.iter().enumerate() {
                println!("  Ping {:2}: {:>10.2?}", i + 1, sample);
            }
            println!();
            print_stats(&stats.samples);
        }
        Err(e) => {
            eprintln!("Measurement failed: {}", e);
        }
    }

    // Stay online for peer
    println!();
    println!("Staying online for peer (30s)...");
    thread::sleep(Duration::from_secs(30));
    println!("Done.");
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
