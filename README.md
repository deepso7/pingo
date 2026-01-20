# Pingo

A Rust library for peer-to-peer latency measurement using STUN and UDP hole punching.

## Features

- **Custom STUN implementation** - Discovers your public IP:port via RFC 5389
- **UDP hole punching** - Establishes direct connections between NAT'd peers
- **Bidirectional latency measurement** - Both peers can measure RTT simultaneously
- **No dependencies on external crates** (except `rand` for transaction IDs)

## Quick Start

```bash
# On both machines, run:
cargo run --example peer_ping

# Each machine displays its public address
# Exchange addresses, then enter the peer's address when prompted
# Both must enter addresses within a few seconds of each other
```

## Example Output

```
════════════════════════════════════════════
  Your address: 203.0.113.45:54321
════════════════════════════════════════════

Share this with your peer, then enter their address.
(Press 'q' to quit)

> 198.51.100.22:12345

Connecting to 198.51.100.22:12345...
Punching hole...
Connected to 198.51.100.22:12345!

Measuring latency...

  Ping  1:    42.78ms
  Ping  2:    40.35ms
  Ping  3:    41.22ms

  ─────────────────────
  Min:    40.35ms
  Avg:    41.45ms
  Max:    42.78ms
```

## Usage Modes

### Interactive P2P Mode (two NAT'd machines)

```bash
cargo run --example peer_ping
```

Both peers run this command, exchange addresses, and enter them simultaneously.

### Server Mode (EC2/VPS with public IP)

```bash
# On the server (e.g., EC2)
cargo run --example peer_ping -- --listen 9999

# On the client
cargo run --example peer_ping
# Then enter: <server-public-ip>:9999
```

## Library Usage

```rust
use std::net::UdpSocket;
use pingo::{get_public_addr_with_socket, punch_hole, measure_latency};

// Bind a socket and discover public address
let socket = UdpSocket::bind("0.0.0.0:0")?;
let my_addr = get_public_addr_with_socket(Some(&socket))?;
println!("My public address: {}", my_addr);

// Exchange addresses with peer out-of-band, then:
let peer_addr = "198.51.100.22:12345".parse()?;

// Punch hole (both peers must do this simultaneously)
punch_hole(&socket, peer_addr)?;

// Measure latency
let stats = measure_latency(&socket, peer_addr, 10)?;
println!("Average RTT: {:?}", stats.avg);
```

## API

### STUN

```rust
// Discover public address (creates new socket)
pub fn get_public_addr() -> Result<SocketAddr>;

// Discover public address using existing socket (preserves NAT mapping)
pub fn get_public_addr_with_socket(socket: Option<&UdpSocket>) -> Result<SocketAddr>;
```

### Hole Punching

```rust
// Punch a UDP hole to peer (10 second timeout)
pub fn punch_hole(socket: &UdpSocket, peer_addr: SocketAddr) -> Result<()>;
```

### Latency Measurement

```rust
// Measure RTT to peer
pub fn measure_latency(socket: &UdpSocket, peer_addr: SocketAddr, count: u32) -> Result<LatencyStats>;

// Respond to incoming pings (for building custom responders)
pub fn respond_to_ping(socket: &UdpSocket) -> Result<Option<SocketAddr>>;

pub struct LatencyStats {
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub samples: Vec<Duration>,
}
```

## How It Works

1. **STUN Query**: Each peer queries a public STUN server (Google's `stun.l.google.com:19302`) to discover their public IP:port as seen from the internet.

2. **Address Exchange**: Peers exchange their public addresses through some out-of-band mechanism (copy/paste, signaling server, etc.).

3. **Hole Punching**: Both peers simultaneously send UDP packets to each other's public addresses. This creates NAT mappings on both sides, allowing bidirectional traffic to flow.

4. **Latency Measurement**: Once connected, peers exchange timestamped ping/pong packets to measure round-trip time.

## Limitations

- **Symmetric NATs**: Hole punching may fail with symmetric NATs (~10-15% of networks), which assign different external ports for different destinations.
- **UDP only**: This library uses UDP. TCP hole punching is significantly more complex and not supported.
- **IPv4 only**: Currently only supports IPv4 addresses.

## Project Structure

```
src/
├── lib.rs           # Public API
├── error.rs         # Error types
├── stun/
│   ├── message.rs   # STUN message encoding/decoding
│   ├── attributes.rs # XOR-MAPPED-ADDRESS parsing
│   └── client.rs    # STUN client
├── punch/
│   └── hole.rs      # UDP hole punching
└── ping/
    └── latency.rs   # RTT measurement

examples/
├── get_public_addr.rs  # Simple STUN query example
└── peer_ping.rs        # Full P2P latency tool
```

## License

MIT
