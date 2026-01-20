//! Pingo - P2P latency measurement library using STUN and UDP hole punching.
//!
//! # Example
//!
//! ```no_run
//! use std::net::UdpSocket;
//! use pingo::{get_public_addr_with_socket, punch_hole, measure_latency};
//!
//! // Get your public address
//! let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
//! let my_addr = get_public_addr_with_socket(Some(&socket)).unwrap();
//! println!("My public address: {}", my_addr);
//!
//! // Exchange addresses with peer, then punch hole and measure latency
//! // let peer_addr = "1.2.3.4:5678".parse().unwrap();
//! // punch_hole(&socket, peer_addr).unwrap();
//! // let stats = measure_latency(&socket, peer_addr, 10).unwrap();
//! ```

pub mod error;
pub mod ping;
pub mod punch;
pub mod stun;

pub use error::{Error, Result};
pub use ping::{measure_latency, respond_to_ping, LatencyStats};
pub use punch::punch_hole;
pub use stun::{get_public_addr, get_public_addr_with_socket};
