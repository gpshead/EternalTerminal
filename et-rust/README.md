# Eternal Terminal - Rust Implementation

This directory contains a Rust port of Eternal Terminal, designed for full interoperability with the C++ implementation.

## Project Goals

1. **100% Protocol Compatibility**: The Rust implementation uses the same wire protocol, encryption, and message formats as the C++ version
2. **Interoperability**: C++ clients can connect to Rust servers and vice versa
3. **Memory Safety**: Leverage Rust's ownership system for safer networking code
4. **Modern Async**: Use tokio for efficient async I/O
5. **Maintainability**: Cleaner, more maintainable codebase with strong types

## Project Structure

```
et-rust/
├── et-proto/          # Protocol buffer definitions
├── et-base/           # Core networking, crypto, packets
├── et-terminal/       # Terminal logic (client, server, handlers)
├── et-client/         # Client binary (et)
├── et-server/         # Server binary (etserver)
├── et-etterminal/     # User terminal binary (etterminal)
└── et-interop/        # Interoperability tests
```

## Current Status

### ✅ Completed
- Protocol buffer code generation
- Core crypto layer (libsodium integration)
- Packet serialization/deserialization
- Encryption/decryption
- Error handling
- Constants and utilities
- Comprehensive test suite (all passing)

### 🚧 In Progress
- Socket abstractions
- Connection management
- Terminal support

### ⏳ Planned
- Full client/server implementation
- Interoperability test suite
- Port forwarding
- HTM (Headless Terminal Multiplexer)

See [RUST_PORT_STATUS.md](./RUST_PORT_STATUS.md) for detailed status.

## Building

### Prerequisites

- Rust 1.70+ (2021 edition)
- Cargo

### Build Commands

```bash
# Build all crates
cargo build

# Build with optimizations
cargo build --release

# Run tests
cargo test

# Run tests for specific crate
cargo test -p et-base

# Build specific binary (when complete)
cargo build --release --bin et-client
cargo build --release --bin et-server
```

## Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_crypto_roundtrip
```

### Integration Tests

```bash
# Run integration tests (when available)
cargo test --test '*'
```

### Interoperability Tests

```bash
# Build both C++ and Rust implementations
# Then run interop tests (when available)
cd ../build && make
cd ../et-rust && cargo build --release
./run_interop_tests.sh
```

## Dependencies

### Major Crates

- **tokio**: Async runtime for networking
- **prost**: Protocol buffer implementation
- **sodiumoxide**: Rust bindings to libsodium for crypto
- **bytes**: Zero-copy byte buffer management
- **nix**: Unix system calls (PTY, terminals, signals)
- **thiserror**: Error handling
- **tracing**: Structured logging

### Dependency Mapping

| C++ Library | Rust Crate |
|-------------|-----------|
| libsodium | sodiumoxide |
| protobuf | prost |
| easylogging++ | tracing |
| ThreadPool | tokio |
| sole | uuid |

## Architecture

### Protocol Compatibility

The Rust implementation maintains 100% wire-protocol compatibility:

1. **Packet Format**: `[encrypted:1][header:1][payload:N]`
2. **Encryption**: XSalsa20-Poly1305 via libsodium crypto_secretbox
3. **Protocol Version**: 6 (same as C++)
4. **Protobuf Messages**: Identical message definitions

### Design Patterns

| C++ Pattern | Rust Equivalent |
|-------------|-----------------|
| Virtual classes | Traits |
| std::shared_ptr | Arc<T> |
| std::mutex | Mutex<T> or parking_lot::Mutex |
| Exceptions | Result<T, E> |
| Templates | Generics |

### Async Model

- C++ uses thread pools and select/poll
- Rust uses tokio async/await
- Benefits: Better scalability, cleaner code, structured concurrency

## Code Examples

### Creating and Encrypting a Packet

```rust
use et_base::{Packet, CryptoHandler, CRYPTO_KEY_BYTES};

// Create crypto handler
let key = vec![0u8; CRYPTO_KEY_BYTES];
let crypto = CryptoHandler::new(&key, 0)?;

// Create packet
let mut packet = Packet::new(42, b"Hello, ET!");

// Encrypt
packet.encrypt(&crypto)?;

// Serialize for transmission
let bytes = packet.to_bytes();
```

### Working with Protocol Buffers

```rust
use et_base::et_proto::{ConnectRequest, Message};

// Create a connect request
let mut request = ConnectRequest::default();
request.client_id = Some("my-client-id".to_string());
request.version = Some(6);

// Serialize to bytes
let bytes = request.encode_to_vec();

// Deserialize
let decoded = ConnectRequest::decode(&bytes[..])?;
```

## Interoperability

### Testing with C++ Implementation

The Rust implementation can be tested against the C++ implementation:

```bash
# Start C++ server
../build/etserver --port 2022

# Connect with Rust client (when available)
cargo run --bin et-client -- localhost

# Or vice versa:
# Start Rust server
cargo run --bin et-server -- --port 2022

# Connect with C++ client
../build/et localhost
```

### Protocol Verification

Interoperability tests verify:
- Packet format compatibility
- Encryption compatibility (same key = same cipher text with same nonce)
- Protobuf message compatibility
- Connection handshake
- Reconnection logic

## Performance

### Expected Characteristics

- **Crypto**: Same performance as C++ (both use libsodium)
- **Networking**: Potentially faster with tokio for many concurrent connections
- **Memory**: Lower memory usage due to ownership system
- **Latency**: Similar to C++ implementation

### Benchmarks (Planned)

- Connection latency
- Throughput (MB/s)
- Reconnection time
- Memory usage
- CPU usage

## Contributing

When contributing to the Rust port:

1. **Maintain compatibility**: Ensure changes don't break interop with C++
2. **Add tests**: All new code should have unit tests
3. **Follow style**: Use `cargo fmt` and `cargo clippy`
4. **Document**: Add rustdoc comments for public APIs
5. **Test interop**: Verify C++ ↔ Rust communication still works

## Development Workflow

```bash
# Check code compiles
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy

# Run tests
cargo test

# Build documentation
cargo doc --open

# Check for unused dependencies
cargo udeps
```

## FAQ

### Why Rust?

- Memory safety without GC overhead
- Better error handling (Result vs exceptions)
- Modern async/await
- Strong type system catches bugs at compile time
- Excellent tooling (cargo, clippy, rustfmt)

### Will this replace the C++ version?

Not immediately. The goal is to have both implementations that are fully interoperable. Over time, the Rust version may become the primary implementation if it proves more maintainable and performant.

### Can I use a Rust client with a C++ server?

Yes! That's the goal. The implementations are protocol-compatible.

### What about performance?

We expect similar performance to C++. The underlying crypto library (libsodium) is the same, and tokio is highly optimized for async I/O.

## License

Apache-2.0 (same as the C++ implementation)

## Links

- [Eternal Terminal (C++)](https://github.com/MisterTea/EternalTerminal)
- [Port Status Document](./RUST_PORT_STATUS.md)
- [Protocol Documentation](../docs/protocol.md)

## Acknowledgments

This Rust port is based on the excellent C++ implementation by Jason Gauci and contributors. The protocol design, crypto approach, and overall architecture are credited to the original ET project.
