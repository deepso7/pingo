use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use pingo::{get_public_addr_with_socket, measure_latency, punch_hole};

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  Server mode (auto-detect public IP via STUN):");
    eprintln!("    cargo run --example peer_ping");
    eprintln!();
    eprintln!("  Server mode (specify port, for EC2/VPS with public IP):");
    eprintln!("    cargo run --example peer_ping -- --listen <port>");
    eprintln!("    Then tell peer to connect to <your-public-ip>:<port>");
    eprintln!();
    eprintln!("  Client mode:");
    eprintln!("    cargo run --example peer_ping -- <peer_ip:port>");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        // No args - server mode with STUN
        run_as_server(None);
    } else if args[1] == "--listen" {
        // --listen <port> - server mode with specific port (for EC2)
        let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
            print_usage();
            std::process::exit(1);
        });
        run_as_server(Some(port));
    } else if args[1] == "--help" || args[1] == "-h" {
        print_usage();
    } else {
        // Peer address provided - client mode
        let peer_addr: SocketAddr = args[1].parse().unwrap_or_else(|_| {
            eprintln!("Invalid peer address: {}", args[1]);
            print_usage();
            std::process::exit(1);
        });
        run_as_client(peer_addr);
    }
}

fn run_as_server(fixed_port: Option<u16>) {
    println!("Starting in server mode...");

    // Bind socket
    let bind_addr = match fixed_port {
        Some(port) => format!("0.0.0.0:{}", port),
        None => "0.0.0.0:0".to_string(),
    };
    let socket = UdpSocket::bind(&bind_addr).expect("Failed to bind socket");
    let local_addr = socket.local_addr().unwrap();
    println!("Local address: {}", local_addr);

    // Get our public address (or show instructions for EC2)
    if fixed_port.is_some() {
        println!();
        println!("Listening on port {} (EC2/VPS mode)", local_addr.port());
        println!("Make sure this UDP port is open in your security group!");
        println!();
        println!("Tell your peer to run:");
        println!(
            "  cargo run --example peer_ping -- <your-public-ip>:{}",
            local_addr.port()
        );
    } else {
        let public_addr =
            get_public_addr_with_socket(Some(&socket)).expect("Failed to get public address");
        println!("Public address (from STUN): {}", public_addr);
        println!();
        println!("Share this with your peer, then run:");
        println!("  cargo run --example peer_ping -- {}", public_addr);
    }

    println!();
    println!("Waiting for peer to connect...");
    println!("(Will print ALL incoming packets for debugging)");
    println!();

    // Set timeout for receiving
    socket.set_read_timeout(Some(Duration::from_secs(1))).ok();

    // Wait for hole punch and respond to pings
    let mut buf = [0u8; 512];
    let punch_magic = b"PINGO_PUNCH";
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                println!(
                    "[RECV] {} bytes from {}: {:?}",
                    len,
                    from,
                    &buf[..len.min(32)]
                );

                // Respond to punch packets
                if len == punch_magic.len() && &buf[..len] == punch_magic {
                    socket.send_to(punch_magic, from).ok();
                    println!("[SEND] Punch response to {}", from);
                }
                // Respond to ping packets
                else if len >= 13 && buf[0] == 0x01 {
                    let mut pong = buf[..len].to_vec();
                    pong[0] = 0x02;
                    socket.send_to(&pong, from).ok();
                    println!("[SEND] Pong to {}", from);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}

fn run_as_client(peer_addr: SocketAddr) {
    println!("Starting in client mode...");
    println!("Peer address: {}", peer_addr);

    // Bind socket
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
    let local_addr = socket.local_addr().unwrap();
    println!("Local address: {}", local_addr);

    // Get our public address (uses same socket, so NAT mapping is preserved)
    let public_addr =
        get_public_addr_with_socket(Some(&socket)).expect("Failed to get public address");

    println!("Public address (from STUN): {}", public_addr);
    println!();

    // Punch hole to peer
    println!("Punching hole to peer...");
    println!("(Sending UDP packets to {} for 10 seconds)", peer_addr);
    match punch_hole(&socket, peer_addr) {
        Ok(()) => println!("Hole punched successfully!"),
        Err(e) => {
            eprintln!("Hole punch failed: {}", e);
            eprintln!("Make sure the peer is running and both started around the same time.");
            return;
        }
    }

    println!();
    println!("Measuring latency (10 pings)...");

    // Measure latency
    match measure_latency(&socket, peer_addr, 10) {
        Ok(stats) => {
            println!();
            println!("Results:");
            println!("  Min: {:?}", stats.min);
            println!("  Max: {:?}", stats.max);
            println!("  Avg: {:?}", stats.avg);
            println!();
            println!("Individual samples:");
            for (i, sample) in stats.samples.iter().enumerate() {
                println!("  Ping {}: {:?}", i + 1, sample);
            }
        }
        Err(e) => {
            eprintln!("Latency measurement failed: {}", e);
        }
    }
}
