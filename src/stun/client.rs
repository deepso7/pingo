use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use crate::Result;

use super::attributes::parse_xor_mapped_address;
use super::message::StunMessage;

/// Google's public STUN server
const STUN_SERVER: &str = "stun.l.google.com:19302";

/// Timeout for STUN request
const STUN_TIMEOUT: Duration = Duration::from_secs(5);

/// Get public address by querying a STUN server
pub fn get_public_addr() -> Result<SocketAddr> {
    get_public_addr_with_socket(None)
}

/// Get public address using a specific socket (useful for hole punching)
pub fn get_public_addr_with_socket(socket: Option<&UdpSocket>) -> Result<SocketAddr> {
    // Resolve STUN server address (prefer IPv4)
    let stun_addr = STUN_SERVER
        .to_socket_addrs()?
        .find(|addr| addr.is_ipv4())
        .ok_or("failed to resolve STUN server (no IPv4 address)")?;

    // Create or use provided socket
    let owned_socket;
    let socket = match socket {
        Some(s) => s,
        None => {
            owned_socket = UdpSocket::bind("0.0.0.0:0")?;
            &owned_socket
        }
    };

    socket.set_read_timeout(Some(STUN_TIMEOUT))?;

    // Create and send binding request
    let request = StunMessage::binding_request();
    let transaction_id = request.transaction_id;
    let encoded = request.encode();

    socket.send_to(&encoded, stun_addr)?;

    // Receive response
    let mut buf = [0u8; 512];
    let (len, _) = socket.recv_from(&mut buf)?;

    // Decode response
    let response = StunMessage::decode(&buf[..len])?;

    // Validate response
    if !response.is_binding_response() {
        return Err(format!("unexpected message type: 0x{:04X}", response.msg_type).into());
    }

    if !response.matches_transaction(&transaction_id) {
        return Err("transaction ID mismatch".into());
    }

    // Extract public address
    parse_xor_mapped_address(&response.attributes, &transaction_id)
}
