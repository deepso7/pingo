//! STUN protocol implementation (RFC 5389 subset).
//!
//! This module provides a minimal STUN client for discovering public IP addresses.

mod attributes;
mod client;
mod message;

pub use client::{get_public_addr, get_public_addr_with_socket};
