use rand::Rng;

use crate::Result;

/// STUN magic cookie (RFC 5389)
pub const MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN message types
pub const BINDING_REQUEST: u16 = 0x0001;
pub const BINDING_RESPONSE: u16 = 0x0101;

/// STUN header size (20 bytes)
pub const HEADER_SIZE: usize = 20;

/// STUN message
#[derive(Debug, Clone)]
pub struct StunMessage {
    pub msg_type: u16,
    pub length: u16,
    pub transaction_id: [u8; 12],
    pub attributes: Vec<u8>,
}

impl StunMessage {
    /// Create a new binding request
    pub fn binding_request() -> Self {
        let mut transaction_id = [0u8; 12];
        rand::thread_rng().fill(&mut transaction_id);

        StunMessage {
            msg_type: BINDING_REQUEST,
            length: 0,
            transaction_id,
            attributes: Vec::new(),
        }
    }

    /// Encode message to bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE + self.attributes.len());

        // Message type (2 bytes)
        buf.extend_from_slice(&self.msg_type.to_be_bytes());

        // Message length (2 bytes) - length of attributes only
        buf.extend_from_slice(&(self.attributes.len() as u16).to_be_bytes());

        // Magic cookie (4 bytes)
        buf.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());

        // Transaction ID (12 bytes)
        buf.extend_from_slice(&self.transaction_id);

        // Attributes
        buf.extend_from_slice(&self.attributes);

        buf
    }

    /// Decode message from bytes
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err("message too short".into());
        }

        let msg_type = u16::from_be_bytes([data[0], data[1]]);
        let length = u16::from_be_bytes([data[2], data[3]]);
        let cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        if cookie != MAGIC_COOKIE {
            return Err(format!("invalid magic cookie: 0x{:08X}", cookie).into());
        }

        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&data[8..20]);

        let attributes = if data.len() > HEADER_SIZE {
            data[HEADER_SIZE..HEADER_SIZE + length as usize].to_vec()
        } else {
            Vec::new()
        };

        Ok(StunMessage {
            msg_type,
            length,
            transaction_id,
            attributes,
        })
    }

    /// Check if this is a binding response
    pub fn is_binding_response(&self) -> bool {
        self.msg_type == BINDING_RESPONSE
    }

    /// Verify transaction ID matches
    pub fn matches_transaction(&self, other: &[u8; 12]) -> bool {
        self.transaction_id == *other
    }
}
