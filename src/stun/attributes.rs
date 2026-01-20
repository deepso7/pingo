//! STUN attribute parsing.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::message::MAGIC_COOKIE;
use crate::Result;

/// XOR-MAPPED-ADDRESS attribute type.
const XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// MAPPED-ADDRESS attribute type (legacy fallback).
const MAPPED_ADDRESS: u16 = 0x0001;

/// IPv4 address family.
const FAMILY_IPV4: u8 = 0x01;

/// IPv6 address family.
const FAMILY_IPV6: u8 = 0x02;

/// Parses XOR-MAPPED-ADDRESS (or MAPPED-ADDRESS) from STUN attributes.
///
/// Returns the public socket address of the client as seen by the STUN server.
pub fn parse_xor_mapped_address(
    attributes: &[u8],
    transaction_id: &[u8; 12],
) -> Result<SocketAddr> {
    let mut offset = 0;

    while offset + 4 <= attributes.len() {
        let attr_type = u16::from_be_bytes([attributes[offset], attributes[offset + 1]]);
        let attr_len =
            u16::from_be_bytes([attributes[offset + 2], attributes[offset + 3]]) as usize;

        offset += 4;

        if attr_type == XOR_MAPPED_ADDRESS {
            return decode_xor_mapped_address(
                &attributes[offset..offset + attr_len],
                transaction_id,
            );
        }

        if attr_type == MAPPED_ADDRESS {
            return decode_mapped_address(&attributes[offset..offset + attr_len]);
        }

        // Attributes are padded to 4-byte boundaries
        offset += (attr_len + 3) & !3;
    }

    Err("XOR-MAPPED-ADDRESS attribute not found".into())
}

/// Decodes XOR-MAPPED-ADDRESS attribute value.
fn decode_xor_mapped_address(data: &[u8], transaction_id: &[u8; 12]) -> Result<SocketAddr> {
    if data.len() < 8 {
        return Err("XOR-MAPPED-ADDRESS too short".into());
    }

    let family = data[1];
    let x_port = u16::from_be_bytes([data[2], data[3]]);
    let port = x_port ^ (MAGIC_COOKIE >> 16) as u16;

    let ip = match family {
        FAMILY_IPV4 => {
            let x_addr = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
            IpAddr::V4(Ipv4Addr::from(x_addr ^ MAGIC_COOKIE))
        }
        FAMILY_IPV6 => {
            if data.len() < 20 {
                return Err("XOR-MAPPED-ADDRESS IPv6 too short".into());
            }
            // XOR with magic cookie (4 bytes) + transaction ID (12 bytes)
            let mut xor_bytes = [0u8; 16];
            xor_bytes[0..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
            xor_bytes[4..16].copy_from_slice(transaction_id);

            let mut addr_bytes = [0u8; 16];
            for i in 0..16 {
                addr_bytes[i] = data[4 + i] ^ xor_bytes[i];
            }
            IpAddr::V6(Ipv6Addr::from(addr_bytes))
        }
        _ => return Err(format!("unknown address family: {}", family).into()),
    };

    Ok(SocketAddr::new(ip, port))
}

/// Decodes MAPPED-ADDRESS attribute value (legacy, non-XOR).
fn decode_mapped_address(data: &[u8]) -> Result<SocketAddr> {
    if data.len() < 8 {
        return Err("MAPPED-ADDRESS too short".into());
    }

    let family = data[1];
    let port = u16::from_be_bytes([data[2], data[3]]);

    let ip = match family {
        FAMILY_IPV4 => {
            let addr = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
            IpAddr::V4(Ipv4Addr::from(addr))
        }
        FAMILY_IPV6 => {
            if data.len() < 20 {
                return Err("MAPPED-ADDRESS IPv6 too short".into());
            }
            let mut addr_bytes = [0u8; 16];
            addr_bytes.copy_from_slice(&data[4..20]);
            IpAddr::V6(Ipv6Addr::from(addr_bytes))
        }
        _ => return Err(format!("unknown address family: {}", family).into()),
    };

    Ok(SocketAddr::new(ip, port))
}
