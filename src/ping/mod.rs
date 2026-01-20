//! Latency measurement using ping/pong packets.

mod latency;

pub use latency::{measure_latency, respond_to_ping, LatencyStats};
