//! STUN message encoding and decoding.

use rand::Rng;

use crate::Result;

/// STUN magic cookie (RFC 5389).
pub const MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN Binding Request message type.
pub const BINDING_REQUEST: u16 = 0x0001;

/// STUN Binding Response message type.
pub const BINDING_RESPONSE: u16 = 0x0101;

/// STUN header size in bytes.
pub const HEADER_SIZE: usize = 20;

/// A STUN message.
#[derive(Debug, Clone)]
pub struct StunMessage {
    /// Message type (request/response).
    pub msg_type: u16,
    /// Length of attributes (stored but computed on encode).
    #[allow(dead_code)]
    pub length: u16,
    /// 96-bit transaction ID.
    pub transaction_id: [u8; 12],
    /// Raw attribute bytes.
    pub attributes: Vec<u8>,
}

impl StunMessage {
    /// Creates a new STUN Binding Request with a random transaction ID.
    pub fn binding_request() -> Self {
        let mut transaction_id = [0u8; 12];
        rand::thread_rng().fill(&mut transaction_id);

        Self {
            msg_type: BINDING_REQUEST,
            length: 0,
            transaction_id,
            attributes: Vec::new(),
        }
    }

    /// Encodes the message to bytes for transmission.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE + self.attributes.len());

        buf.extend_from_slice(&self.msg_type.to_be_bytes());
        buf.extend_from_slice(&(self.attributes.len() as u16).to_be_bytes());
        buf.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        buf.extend_from_slice(&self.transaction_id);
        buf.extend_from_slice(&self.attributes);

        buf
    }

    /// Decodes a STUN message from received bytes.
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err("STUN message too short".into());
        }

        let msg_type = u16::from_be_bytes([data[0], data[1]]);
        let length = u16::from_be_bytes([data[2], data[3]]);
        let cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        if cookie != MAGIC_COOKIE {
            return Err(format!("invalid STUN magic cookie: 0x{:08X}", cookie).into());
        }

        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&data[8..20]);

        let attr_end = HEADER_SIZE + length as usize;
        let attributes = if data.len() >= attr_end {
            data[HEADER_SIZE..attr_end].to_vec()
        } else {
            Vec::new()
        };

        Ok(Self {
            msg_type,
            length,
            transaction_id,
            attributes,
        })
    }

    /// Returns true if this is a Binding Response.
    pub fn is_binding_response(&self) -> bool {
        self.msg_type == BINDING_RESPONSE
    }

    /// Returns true if the transaction ID matches.
    pub fn matches_transaction(&self, other: &[u8; 12]) -> bool {
        self.transaction_id == *other
    }
}
