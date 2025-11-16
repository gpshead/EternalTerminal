# Eternal Terminal Production Implementation Plan

## Overview

This document outlines the architecture and implementation plan for a production-ready Rust implementation of Eternal Terminal that is fully compatible with the C++ implementation.

## Architecture Components

### 1. Core Library (`et-base`) - ✅ COMPLETE

Already implemented:
- Crypto handling (XSalsa20-Poly1305)
- Packet serialization/deserialization
- Socket abstractions (TCP, async)
- Backed reader/writer (reliable transport)
- Connection management
- Protocol constants

### 2. Protocol Buffers (`et-proto`) - ✅ COMPLETE

Already implemented:
- ET.proto and ETerminal.proto compiled
- All protocol message types available

### 3. Terminal Library (`et-terminal`) - 🚧 TO IMPLEMENT

**Purpose**: Core terminal handling functionality

**Modules to Implement**:

#### `pty.rs` - PTY Management
```rust
pub struct PtyMaster {
    fd: RawFd,
    size: TerminalSize,
}

impl PtyMaster {
    pub fn open() -> Result<Self>;
    pub fn set_size(&mut self, rows: u16, cols: u16) -> Result<()>;
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    pub fn write(&self, buf: &[u8]) -> Result<usize>;
    pub fn spawn_shell(&self) -> Result<Pid>;
}
```

**Dependencies**: `nix` crate for:
- `posix_openpt()` - open PTY master
- `grantpt()` / `unlockpt()` - PTY setup
- `ptsname()` - get slave PTY name
- `fork()` / `setsid()` / `dup2()` - spawn process in PTY
- `tcgetattr()` / `tcsetattr()` - terminal attributes
- `ioctl(TIOCSWINSZ)` - set terminal size

#### `raw_mode.rs` - Terminal Raw Mode
```rust
pub struct RawModeGuard {
    original_termios: Termios,
    fd: RawFd,
}

impl RawModeGuard {
    pub fn enable(fd: RawFd) -> Result<Self>;
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        // Restore original terminal settings
    }
}
```

#### `terminal_client.rs` - Client Terminal Logic
```rust
pub struct TerminalClient {
    connection: Arc<ClientConnection>,
    local_pty: Option<PtyMaster>,
    remote_session_id: String,
}

impl TerminalClient {
    pub async fn connect(host: &str, port: u16, passkey: &str) -> Result<Self>;
    pub async fn run_interactive(&mut self) -> Result<()>;
    pub async fn run_command(&mut self, command: &str) -> Result<i32>;
}
```

#### `terminal_server.rs` - Server Terminal Logic
```rust
pub struct TerminalServer {
    listener: Box<dyn AsyncListener>,
    sessions: Arc<Mutex<HashMap<String, UserSession>>>,
}

pub struct UserSession {
    client_id: String,
    connection: Arc<ServerClientConnection>,
    user_terminal: Option<UserTerminalHandle>,
}

impl TerminalServer {
    pub async fn new(bind_addr: &str, port: u16) -> Result<Self>;
    pub async fn run(&mut self) -> Result<()>;
}
```

### 4. User Terminal (`et-etterminal`) - 🚧 TO IMPLEMENT

**Purpose**: User terminal process (equivalent to C++ `etterminal`)

**Binary**: `etterminal-rs`

**Flow**:
1. Spawned by client via SSH
2. Receives id/passkey via stdin or environment
3. Opens PTY and spawns user shell
4. Connects to ET server on localhost
5. Bridges PTY ↔ ET connection

**Implementation**:

```rust
// et-etterminal/src/main.rs
use et_base::ClientConnection;
use et_terminal::PtyMaster;

#[derive(Parser)]
struct Args {
    /// Client ID (passed from et client)
    #[arg(long)]
    client_id: String,

    /// Passkey (passed from et client)
    #[arg(long)]
    passkey: String,

    /// ET server address
    #[arg(long, default_value = "127.0.0.1")]
    server: String,

    /// ET server port
    #[arg(long, default_value = "2022")]
    port: u16,

    /// Command to run (if not interactive shell)
    #[arg(long)]
    command: Option<String>,
}

async fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Connect to ET server
    let connection = ClientConnection::connect(
        &args.server,
        args.port,
        &args.client_id,
        &args.passkey
    ).await?;

    // 2. Create PTY and spawn shell
    let mut pty = PtyMaster::open()?;
    let shell = std::env::var("SHELL").unwrap_or("/bin/bash".to_string());
    pty.spawn_shell(&shell, args.command.as_deref())?;

    // 3. Bridge PTY ↔ Connection
    bridge_pty_connection(pty, connection).await?;

    Ok(())
}

async fn bridge_pty_connection(
    mut pty: PtyMaster,
    connection: Arc<ClientConnection>
) -> Result<()> {
    // Spawn two tasks:
    // - PTY → Connection (forward user input to server)
    // - Connection → PTY (forward server output to terminal)

    tokio::select! {
        result = pty_to_connection(&mut pty, &connection) => result,
        result = connection_to_pty(&connection, &mut pty) => result,
    }
}
```

### 5. Production Client (`et-client` enhanced) - 🚧 TO IMPLEMENT

**Purpose**: Full terminal client with SSH integration

**New Binary**: `et` (production client, different from `etclient-rs` test tool)

**Flow**:
1. Parse connection arguments (user@host, port, jumphost, etc.)
2. Read SSH config (~/.ssh/config) for host settings
3. Generate random passkey
4. SSH to remote host and spawn `etterminal` with id/passkey
5. Connect to remote ET server
6. Enter raw mode on local terminal
7. Bridge local terminal ↔ ET connection
8. Handle reconnection on network interruption

**SSH Integration Options**:

**Option A: Use `russh` crate** (pure Rust, async-first)
```toml
[dependencies]
russh = "0.40"
russh-keys = "0.40"
```

**Option B: Use `ssh2` crate** (libssh2 bindings, mature)
```toml
[dependencies]
ssh2 = "0.9"
```

**Recommendation**: Start with `ssh2` for stability, consider `russh` later.

**Implementation Sketch**:

```rust
// New binary: et-rust/bins/et/main.rs
use et_base::ClientConnection;
use et_terminal::{TerminalClient, RawModeGuard};
use ssh2::Session;

#[derive(Parser)]
struct Args {
    /// Target: [user@]host[:port]
    target: String,

    /// Command to run (if not interactive)
    #[arg(short = 't')]
    command: Option<String>,

    /// Jumphost
    #[arg(short = 'J')]
    jumphost: Option<String>,

    /// ET server port
    #[arg(long, default_value = "2022")]
    port: u16,

    /// SSH options
    #[arg(short = 'o', value_name = "option")]
    ssh_options: Vec<String>,
}

async fn main() -> Result<()> {
    let args = Args::parse();

    // Parse target
    let (user, host, ssh_port) = parse_target(&args.target)?;

    // Generate client ID and passkey
    let client_id = Uuid::new_v4().to_string();
    let passkey = generate_passkey(); // 32-char random

    // 1. SSH to remote host
    let ssh_session = establish_ssh(&user, &host, ssh_port, &args.ssh_options)?;

    // 2. Spawn etterminal on remote host
    let etterminal_cmd = format!(
        "etterminal --client-id {} --passkey {} --server 127.0.0.1 --port {}",
        client_id, passkey, args.port
    );

    if let Some(cmd) = &args.command {
        etterminal_cmd.push_str(&format!(" --command '{}'", cmd));
    }

    let mut remote_channel = ssh_session.channel_session()?;
    remote_channel.exec(&etterminal_cmd)?;

    // Give etterminal time to connect to server
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 3. Connect ET client to server
    let mut terminal_client = TerminalClient::connect(
        &host,
        args.port,
        &client_id,
        &passkey
    ).await?;

    // 4. Enter raw mode and run interactive session
    let _raw_guard = RawModeGuard::enable(STDIN_FILENO)?;
    terminal_client.run_interactive().await?;

    Ok(())
}
```

### 6. Production Server (`et-server` enhanced) - 🚧 TO IMPLEMENT

**Purpose**: Full terminal server with user session management

**New Binary**: `etserver` (production server, different from `etserver-rs` test tool)

**Flow**:
1. Listen on configured port
2. Accept client connections
3. Perform handshake and retrieve passkey
4. For new clients: expect `etterminal` to connect with matching id/passkey
5. For returning clients: reconnect to existing session
6. Bridge client connection ↔ user terminal
7. Handle terminal resize events
8. Manage session persistence

**Architecture**:

```rust
// New binary: et-rust/bins/etserver/main.rs
use et_base::{ServerConnection, ServerClientConnection};
use et_terminal::TerminalServer;

#[derive(Parser)]
struct Args {
    /// Port to listen on
    #[arg(long, default_value = "2022")]
    port: u16,

    /// Bind address
    #[arg(long, default_value = "0.0.0.0")]
    bindip: String,

    /// Daemonize
    #[arg(long)]
    daemon: bool,

    /// PID file
    #[arg(long, default_value = "/var/run/etserver.pid")]
    pidfile: String,

    /// Log directory
    #[arg(short = 'l', long)]
    logdir: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.daemon {
        daemonize(&args.pidfile)?;
    }

    let mut server = TerminalServer::new(&args.bindip, args.port).await?;
    server.run().await?;

    Ok(())
}
```

## Implementation Phases

### Phase 1: PTY and Terminal Basics (Week 1)
- [x] et-base library (already complete)
- [ ] Implement `pty.rs` - PTY creation and management
- [ ] Implement `raw_mode.rs` - Terminal raw mode handling
- [ ] Unit tests for PTY operations
- [ ] Simple PTY demo: spawn shell in PTY, echo I/O

### Phase 2: User Terminal Process (Week 2)
- [ ] Implement `etterminal-rs` binary
- [ ] PTY ↔ Connection bridging logic
- [ ] Terminal size change handling (SIGWINCH)
- [ ] Environment variable passing
- [ ] Test: Manual spawn of etterminal → connects to test server

### Phase 3: SSH Integration (Week 3)
- [ ] Add `ssh2` dependency
- [ ] Implement SSH client connection
- [ ] SSH config file parsing (~/.ssh/config)
- [ ] SSH key authentication
- [ ] Remote command execution
- [ ] Test: SSH to remote host, run command, get output

### Phase 4: Production Client (Week 4)
- [ ] Implement production `et` binary
- [ ] Target parsing (user@host:port)
- [ ] Passkey generation and passing
- [ ] SSH → spawn etterminal → ET connect flow
- [ ] Local terminal raw mode
- [ ] Local terminal ↔ ET connection bridging
- [ ] Reconnection handling
- [ ] Test: `et user@localhost` opens interactive shell

### Phase 5: Production Server (Week 5)
- [ ] Implement production `etserver` binary
- [ ] Session management (id → user terminal mapping)
- [ ] Etterminal connection acceptance
- [ ] Client connection acceptance
- [ ] Session bridging (client ↔ etterminal)
- [ ] Daemon mode
- [ ] Test: Full flow with production binaries

### Phase 6: Advanced Features (Week 6+)
- [ ] Port forwarding
- [ ] Jumphost support
- [ ] HTM (headless terminal multiplexer) mode
- [ ] Telemetry and logging
- [ ] Configuration file support

### Phase 7: C++ Interoperability Testing (Week 7)
- [ ] Test: Rust `et` client → C++ `etserver`
- [ ] Test: C++ `et` client → Rust `etserver`
- [ ] Test: Mixed jumphosts (Rust → C++ → destination)
- [ ] Performance benchmarking
- [ ] Protocol compatibility validation

## Dependencies to Add

```toml
[workspace.dependencies]
# SSH (add to existing dependencies)
ssh2 = "0.9"

# Additional utilities
dirs = "5.0"  # For home directory, config paths
shellexpand = "3.0"  # For ~/ expansion
signal-hook = "0.3"  # For SIGWINCH handling
daemonize = "0.5"  # For daemon mode
```

## Project Structure

```
et-rust/
├── et-base/           # Core protocol (COMPLETE)
├── et-proto/          # Protocol buffers (COMPLETE)
├── et-terminal/       # Terminal library (TO IMPLEMENT)
│   └── src/
│       ├── lib.rs
│       ├── pty.rs          # PTY management
│       ├── raw_mode.rs     # Terminal raw mode
│       ├── terminal_client.rs  # Client terminal logic
│       ├── terminal_server.rs  # Server terminal logic
│       └── utils.rs        # Terminal utilities
├── bins/              # Production binaries (TO CREATE)
│   ├── et/            # Production client
│   │   └── main.rs
│   ├── etserver/      # Production server
│   │   └── main.rs
│   └── etterminal/    # User terminal
│       └── main.rs
├── et-client/         # Test client (KEEP AS-IS)
├── et-server/         # Test server (KEEP AS-IS)
└── tests/             # Integration tests
    ├── pty_tests.rs
    ├── ssh_tests.rs
    └── interop_tests.rs
```

## Testing Strategy

### Unit Tests
- PTY operations (open, read, write, resize)
- Terminal mode switching
- SSH connection establishment
- Passkey generation

### Integration Tests
- Etterminal → Test server connection
- Full flow: client → SSH → etterminal → server
- Reconnection scenarios
- Terminal resize handling

### Interop Tests
- Rust client → C++ server
- C++ client → Rust server
- Protocol compatibility
- Performance comparison

## Success Criteria

1. ✅ **Protocol Compatibility**: Wire-level compatibility with C++ (DONE)
2. 🎯 **Feature Parity**: All core ET features implemented
3. 🎯 **Interoperability**: Rust client works with C++ server and vice versa
4. 🎯 **Production Ready**: Stable, tested, documented
5. 🎯 **Performance**: Match or exceed C++ performance

## Timeline Estimate

- **Minimum Viable Product**: 4-5 weeks
  - PTY + etterminal + basic client/server
  - Interactive shell sessions working
  - Reconnection support

- **Feature Complete**: 6-7 weeks
  - All core ET features
  - C++ interoperability verified
  - Production hardening

- **Polish & Release**: 8-10 weeks
  - Performance optimization
  - Comprehensive testing
  - Documentation
  - Packaging

## Current Status

- ✅ Phase 0: Protocol implementation and test binaries (COMPLETE)
- 🚧 Phase 1: PTY and terminal basics (STARTING)

## Next Steps

1. Implement `et-terminal/src/pty.rs` with basic PTY operations
2. Create simple PTY demo to validate functionality
3. Implement `etterminal-rs` binary
4. Test etterminal → etserver-rs connection
5. Add SSH integration
6. Build production client
7. Build production server
8. Full integration testing
9. C++ interoperability testing

---

**Document Version**: 1.0
**Last Updated**: 2025-11-16
**Status**: Planning → Implementation
