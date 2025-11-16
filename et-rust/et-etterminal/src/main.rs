/// ET User Terminal Process (etterminal-rs)
///
/// This is the user terminal process that runs on the server side. It:
/// 1. Creates a PTY (pseudo-terminal) and spawns a shell
/// 2. Connects to the ET server
/// 3. Bridges PTY I/O ↔ ET connection bidirectionally
/// 4. Handles terminal resize events (SIGWINCH)
///
/// In production, this process is spawned by the et client via SSH.
/// For testing, it can be run manually with a client ID and passkey.

use anyhow::{Context, Result};
use bytes::BytesMut;
use clap::Parser;
use et_base::{
    socket::{self, AsyncSocket, SocketHandler, TcpSocketHandler},
    CryptoHandler, Packet, CLIENT_SERVER_NONCE_MSB, PROTOCOL_VERSION, SERVER_CLIENT_NONCE_MSB,
};
use et_proto::{ConnectRequest, ConnectResponse, ConnectStatus, SocketEndpoint};
use et_terminal::{PtyMaster, TerminalSize};
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

#[derive(Parser, Debug)]
#[command(name = "etterminal-rs")]
#[command(about = "Eternal Terminal User Terminal Process", long_about = None)]
struct Args {
    /// Client ID (passed from et client)
    #[arg(long)]
    client_id: String,

    /// Passkey (32-byte hex or 32-char ASCII, passed from et client)
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

    /// Terminal rows (for initial size)
    #[arg(long)]
    rows: Option<u16>,

    /// Terminal columns (for initial size)
    #[arg(long)]
    cols: Option<u16>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

/// Parse passkey from string (hex or ASCII) to 32-byte key
fn parse_passkey(passkey: &str) -> Result<Vec<u8>> {
    // Try to parse as hex first (64 hex chars = 32 bytes)
    if passkey.len() == 64 && passkey.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut key = Vec::with_capacity(32);
        for i in 0..32 {
            let byte_str = &passkey[i * 2..i * 2 + 2];
            let byte = u8::from_str_radix(byte_str, 16)
                .context("Invalid hex passkey")?;
            key.push(byte);
        }
        Ok(key)
    } else if passkey.len() == 32 {
        // Treat as 32-character ASCII string (like C++ implementation)
        Ok(passkey.as_bytes().to_vec())
    } else {
        anyhow::bail!(
            "Passkey must be either 64 hex characters or 32 ASCII characters, got {} chars",
            passkey.len()
        )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    info!("Starting ET User Terminal v{}", env!("CARGO_PKG_VERSION"));
    info!("Client ID: {}", args.client_id);
    info!("Connecting to ET server: {}:{}", args.server, args.port);

    // Parse passkey
    let key = parse_passkey(&args.passkey)
        .context("Failed to parse passkey")?;

    // Create PTY and spawn shell
    let pty = create_pty_and_spawn_shell(&args)
        .context("Failed to create PTY and spawn shell")?;

    // Connect to ET server
    let socket = connect_to_server(&args, &key).await
        .context("Failed to connect to ET server")?;

    info!("Connected to ET server, entering bridge loop");

    // Set up signal handling for SIGWINCH (terminal resize)
    let signals = Signals::new(&[SIGWINCH])
        .context("Failed to set up signal handling")?;
    let signal_handle = signals.handle();

    let pty = Arc::new(Mutex::new(pty));
    let pty_clone = pty.clone();

    // Spawn signal handler task
    let signal_task = tokio::spawn(async move {
        handle_signals(signals, pty_clone).await;
    });

    // Run bidirectional bridge: PTY ↔ Connection
    let result = bridge_pty_connection(pty, socket, &key).await;

    // Clean up
    signal_handle.close();
    signal_task.abort();

    match result {
        Ok(_) => {
            info!("Terminal session ended normally");
            Ok(())
        }
        Err(e) => {
            error!("Terminal session ended with error: {}", e);
            Err(e)
        }
    }
}

/// Create PTY and spawn shell
fn create_pty_and_spawn_shell(args: &Args) -> Result<PtyMaster> {
    let mut pty = PtyMaster::open()
        .context("Failed to open PTY")?;

    // Set initial terminal size
    if let (Some(rows), Some(cols)) = (args.rows, args.cols) {
        pty.set_size(rows, cols)
            .context("Failed to set PTY size")?;
        info!("Set initial terminal size: {}x{}", rows, cols);
    }

    // Get shell from environment or use default
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    info!("Spawning shell: {}", shell);

    // Prepare environment variables
    let mut env_vars = vec![];

    // Pass through important environment variables
    for var in &["USER", "HOME", "PATH", "LANG", "LC_ALL"] {
        if let Ok(value) = std::env::var(var) {
            env_vars.push((var.to_string(), value));
        }
    }

    // Ensure TERM is set
    if std::env::var("TERM").is_err() {
        env_vars.push(("TERM".to_string(), "xterm-256color".to_string()));
    }

    // Spawn shell in PTY
    pty.spawn_shell(&shell, args.command.as_deref(), Some(&env_vars))
        .context("Failed to spawn shell in PTY")?;

    Ok(pty)
}

/// Connect to ET server and perform handshake
async fn connect_to_server(args: &Args, key: &[u8]) -> Result<Box<dyn AsyncSocket>> {
    let socket_handler = Arc::new(TcpSocketHandler::new());
    let mut endpoint = SocketEndpoint::default();
    endpoint.name = Some(args.server.clone());
    endpoint.port = Some(args.port as i32);

    // Connect to server
    debug!("Connecting to {}:{}", args.server, args.port);
    let mut socket = socket_handler.connect(&endpoint).await
        .context("Failed to connect to ET server")?;

    // Send connect request
    debug!("Sending connect request");
    let mut request = ConnectRequest::default();
    request.client_id = Some(args.client_id.clone());
    request.version = Some(PROTOCOL_VERSION);

    socket::write_proto(&mut *socket, &request).await
        .context("Failed to send connect request")?;

    // Receive connect response
    debug!("Waiting for connect response");
    let response: ConnectResponse = socket::read_proto(&mut *socket).await
        .context("Failed to receive connect response")?;

    let status = response.status.unwrap_or(ConnectStatus::InvalidKey as i32);
    if status != ConnectStatus::NewClient as i32
        && status != ConnectStatus::ReturningClient as i32
    {
        let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
        anyhow::bail!("Server rejected connection: status={}, error={}", status, error);
    }

    let status_str = if status == ConnectStatus::NewClient as i32 {
        "NEW_CLIENT"
    } else {
        "RETURNING_CLIENT"
    };
    info!("Connection accepted: {}", status_str);

    Ok(socket)
}

/// Bridge PTY and ET connection bidirectionally
async fn bridge_pty_connection(
    pty: Arc<Mutex<PtyMaster>>,
    mut socket: Box<dyn AsyncSocket>,
    key: &[u8],
) -> Result<()> {
    // Create crypto handlers with the provided key

    let reader_crypto = CryptoHandler::new(&key, SERVER_CLIENT_NONCE_MSB)
        .context("Failed to create reader crypto")?;
    let writer_crypto = CryptoHandler::new(&key, CLIENT_SERVER_NONCE_MSB)
        .context("Failed to create writer crypto")?;

    let reader_crypto = Arc::new(Mutex::new(reader_crypto));
    let writer_crypto = Arc::new(Mutex::new(writer_crypto));

    // Create buffers for both directions
    let mut pty_buffer = vec![0u8; 8192];
    let mut socket_buffer = BytesMut::with_capacity(8192);

    loop {
        tokio::select! {
            // PTY → Socket: Read from PTY, write to socket
            result = async {
                let mut pty_guard = pty.lock().await;
                pty_guard.read(&mut pty_buffer)
            } => {
                match result {
                    Ok(0) => {
                        // No data available (non-blocking read)
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Ok(n) => {
                        debug!("Read {} bytes from PTY", n);

                        // Create packet with PTY data
                        let packet = Packet::new(1, pty_buffer[..n].to_vec());

                        // Encrypt packet
                        let mut encrypted_packet = packet;
                        {
                            let crypto = writer_crypto.lock().await;
                            encrypted_packet.encrypt(&crypto)
                                .context("Failed to encrypt packet")?;
                        }

                        // Send packet
                        let serialized = encrypted_packet.to_bytes();
                        let header = (serialized.len() as u32).to_be_bytes();

                        socket.write_all(&header).await
                            .context("Failed to write packet header")?;
                        socket.write_all(&serialized).await
                            .context("Failed to write packet data")?;

                        debug!("Sent {} bytes to socket", n);
                    }
                    Err(e) => {
                        warn!("PTY read error: {}", e);
                        break;
                    }
                }
            },

            // Socket → PTY: Read from socket, write to PTY
            result = async {
                // Read packet length
                let mut len_buf = [0u8; 4];
                socket.read_exact(&mut len_buf).await?;
                let length = u32::from_be_bytes(len_buf) as usize;

                if length == 0 || length > 10 * 1024 * 1024 {
                    anyhow::bail!("Invalid packet length: {}", length);
                }

                // Read packet data
                socket_buffer.clear();
                socket_buffer.resize(length, 0);
                socket.read_exact(&mut socket_buffer).await?;

                Ok::<_, anyhow::Error>(length)
            } => {
                match result {
                    Ok(length) => {
                        debug!("Read {} bytes from socket", length);

                        // Deserialize and decrypt packet
                        let mut packet = Packet::from_bytes(&socket_buffer)
                            .context("Failed to deserialize packet")?;

                        {
                            let crypto = reader_crypto.lock().await;
                            packet.decrypt(&crypto)
                                .context("Failed to decrypt packet")?;
                        }

                        // Write payload to PTY
                        let payload = packet.payload();
                        if !payload.is_empty() {
                            let pty_guard = pty.lock().await;
                            pty_guard.write(payload)
                                .context("Failed to write to PTY")?;
                            debug!("Wrote {} bytes to PTY", payload.len());
                        }
                    }
                    Err(e) => {
                        if e.to_string().contains("unexpected end of file") {
                            info!("Connection closed by remote");
                        } else {
                            warn!("Socket read error: {}", e);
                        }
                        break;
                    }
                }
            },

            // Check if child process is still alive
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                let pty_guard = pty.lock().await;
                if !pty_guard.is_child_alive() {
                    info!("Child process exited");
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Handle signals (SIGWINCH for terminal resize)
async fn handle_signals(mut signals: Signals, pty: Arc<Mutex<PtyMaster>>) {
    use futures::stream::StreamExt;

    while let Some(signal) = signals.next().await {
        match signal {
            SIGWINCH => {
                debug!("Received SIGWINCH, updating PTY size");

                // Get current terminal size from stdin
                if let Ok(size) = TerminalSize::from_stdin() {
                    let mut pty_guard = pty.lock().await;
                    if let Err(e) = pty_guard.set_size(size.rows, size.cols) {
                        warn!("Failed to update PTY size: {}", e);
                    } else {
                        info!("Updated PTY size: {}x{}", size.rows, size.cols);
                    }
                }
            }
            _ => {
                debug!("Received signal: {}", signal);
            }
        }
    }
}
