//! STUN client for discovering public addresses.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use super::attributes::parse_xor_mapped_address;
use super::message::StunMessage;
use crate::Result;

/// Google's public STUN server.
const STUN_SERVER: &str = "stun.l.google.com:19302";

/// Timeout for STUN requests.
const STUN_TIMEOUT: Duration = Duration::from_secs(5);

/// Discovers the public address by querying a STUN server.
///
/// Creates a new UDP socket for the query.
///
/// # Example
///
/// ```no_run
/// let addr = pingo::get_public_addr().unwrap();
/// println!("My public address: {}", addr);
/// ```
pub fn get_public_addr() -> Result<SocketAddr> {
    get_public_addr_with_socket(None)
}

/// Discovers the public address using a specific socket.
///
/// This is useful for hole punching, where you need to preserve the NAT mapping
/// by using the same socket for the STUN query and subsequent peer communication.
///
/// # Example
///
/// ```no_run
/// use std::net::UdpSocket;
///
/// let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
/// let addr = pingo::get_public_addr_with_socket(Some(&socket)).unwrap();
/// println!("My public address: {}", addr);
/// // Now use `socket` to communicate with peers
/// ```
pub fn get_public_addr_with_socket(socket: Option<&UdpSocket>) -> Result<SocketAddr> {
    // Resolve STUN server (prefer IPv4)
    let stun_addr = STUN_SERVER
        .to_socket_addrs()?
        .find(|addr| addr.is_ipv4())
        .ok_or("failed to resolve STUN server")?;

    // Use provided socket or create a new one
    let owned_socket;
    let socket = match socket {
        Some(s) => s,
        None => {
            owned_socket = UdpSocket::bind("0.0.0.0:0")?;
            &owned_socket
        }
    };

    socket.set_read_timeout(Some(STUN_TIMEOUT))?;

    // Send Binding Request
    let request = StunMessage::binding_request();
    let transaction_id = request.transaction_id;
    socket.send_to(&request.encode(), stun_addr)?;

    // Receive response
    let mut buf = [0u8; 512];
    let (len, _) = socket.recv_from(&mut buf)?;

    // Decode and validate response
    let response = StunMessage::decode(&buf[..len])?;

    if !response.is_binding_response() {
        return Err(format!("unexpected STUN message type: 0x{:04X}", response.msg_type).into());
    }

    if !response.matches_transaction(&transaction_id) {
        return Err("STUN transaction ID mismatch".into());
    }

    parse_xor_mapped_address(&response.attributes, &transaction_id)
}
