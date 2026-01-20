# Pingo Implementation Plan

A Rust library for peer-to-peer latency measurement using STUN hole punching.

## Overview

- **Sync only** - `std::net::UdpSocket`, blocking I/O
- **Simple errors** - `Box<dyn Error>` for now
- **Hardcoded STUN server** - `stun.l.google.com:19302`
- **10s hole punch timeout**
- **Configurable ping count**

## Project Structure

```
src/
├── lib.rs              # Re-exports, public API
├── error.rs            # Error/Result types
├── stun/
│   ├── mod.rs          # Module exports
│   ├── message.rs      # STUN message encode/decode
│   ├── attributes.rs   # XOR-MAPPED-ADDRESS parsing
│   └── client.rs       # Query STUN server
├── punch/
│   ├── mod.rs          
│   └── hole.rs         # UDP hole punching
└── ping/
    ├── mod.rs          
    └── latency.rs      # RTT measurement

examples/
├── get_public_addr.rs  # Discover public IP:port
└── peer_ping.rs        # Full peer-to-peer ping test
```

## Dependencies

```toml
[dependencies]
rand = "0.8"
```

## Implementation Steps

### Step 1: Setup

- Create `error.rs` with `Error` and `Result` types
- Fix `Cargo.toml` edition (2024 → 2021)
- Add `rand` dependency

### Step 2: STUN Message (RFC 5389)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|0 0|     STUN Message Type     |         Message Length        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Magic Cookie                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                     Transaction ID (96 bits)                  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- 20-byte header
- Magic cookie: `0x2112A442`
- Random 96-bit transaction ID
- Binding Request type: `0x0001`
- Binding Response type: `0x0101`

### Step 3: STUN Attributes

Parse `XOR-MAPPED-ADDRESS` (type `0x0020`):

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|0 0 0 0 0 0 0 0|    Family     |         X-Port                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                X-Address (32 bits for IPv4)                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- X-Port = Port XOR (magic cookie >> 16)
- X-Address = Address XOR magic cookie

### Step 4: STUN Client

1. Create binding request with random transaction ID
2. Send UDP to `stun.l.google.com:19302`
3. Receive response, validate transaction ID
4. Extract `XOR-MAPPED-ADDRESS` → return `SocketAddr`

### Step 5: `get_public_addr` Example

```
$ cargo run --example get_public_addr
Your public address: 203.0.113.45:54321
```

### Step 6: Hole Punching

1. Both peers know each other's public `IP:port` (manual exchange)
2. Both bind a local UDP socket
3. Both simultaneously send packets to each other's public address
4. NAT creates mapping; when packets cross, hole is punched
5. 10 second timeout

### Step 7: Latency Measurement

Ping protocol:
- Ping packet: `[0x01][8-byte timestamp (nanos since epoch)]`
- Pong packet: `[0x02][original timestamp]`
- Calculate RTT on pong receipt

```rust
pub struct LatencyStats {
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub samples: Vec<Duration>,
}
```

### Step 8: `peer_ping` Example

```
$ cargo run --example peer_ping -- <peer_public_addr>
Punching hole to 198.51.100.22:12345...
Connected!
Ping 1: 23.4ms
Ping 2: 21.1ms
...
Average: 22.1ms
```

## Public API

```rust
// Get public address via STUN
pub fn get_public_addr() -> Result<SocketAddr>;

// Punch hole to peer (10s timeout)
pub fn punch_hole(socket: &UdpSocket, peer_addr: SocketAddr) -> Result<()>;

// Measure latency
pub fn measure_latency(
    socket: &UdpSocket, 
    peer_addr: SocketAddr, 
    count: u32
) -> Result<LatencyStats>;

pub struct LatencyStats {
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub samples: Vec<Duration>,
}
```
