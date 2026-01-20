use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use pingo::{get_public_addr_with_socket, measure_latency, punch_hole, respond_to_ping};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        // No peer address provided - show our public address and wait for pings
        run_as_server();
    } else {
        // Peer address provided - connect and measure latency
        let peer_addr: SocketAddr = args[1].parse().expect("Invalid peer address");
        run_as_client(peer_addr);
    }
}

fn run_as_server() {
    println!("Starting in server mode...");

    // Bind socket
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");

    // Get our public address
    let public_addr =
        get_public_addr_with_socket(Some(&socket)).expect("Failed to get public address");

    println!("Your public address: {}", public_addr);
    println!("Share this with your peer, then run:");
    println!("  cargo run --example peer_ping -- {}", public_addr);
    println!();
    println!("Waiting for peer to connect...");

    // Set timeout for receiving
    socket.set_read_timeout(Some(Duration::from_secs(1))).ok();

    // Wait for hole punch and respond to pings
    loop {
        match respond_to_ping(&socket) {
            Ok(Some(peer)) => {
                println!("Received ping from: {}", peer);
            }
            Ok(None) => {
                // Timeout or non-ping packet, continue
            }
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

    // Get our public address (uses same socket, so NAT mapping is preserved)
    let public_addr =
        get_public_addr_with_socket(Some(&socket)).expect("Failed to get public address");

    println!("Your public address: {}", public_addr);
    println!();

    // Punch hole to peer
    println!("Punching hole to peer...");
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
