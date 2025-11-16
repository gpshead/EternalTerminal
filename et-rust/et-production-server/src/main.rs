/// Production ET Server
///
/// This is the production server that manages terminal sessions.
/// Architecture:
/// - Accepts two types of connections per session:
///   1. Terminal connection (from etterminal-rs on remote server)
///   2. Client connection (from et-rs on user's local machine)
/// - Bridges the two connections bidirectionally
/// - Handles reconnections and session persistence

use anyhow::{Context, Result};
use bytes::BytesMut;
use clap::Parser;
use et_base::{
    socket::{self, AsyncSocket, SocketHandler, TcpSocketHandler},
    CryptoHandler, Packet, CLIENT_SERVER_NONCE_MSB, PROTOCOL_VERSION, SERVER_CLIENT_NONCE_MSB,
};
use et_proto::{ConnectRequest, ConnectResponse, ConnectStatus, SocketEndpoint};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

#[derive(Parser, Debug)]
#[command(name = "etserver-prod")]
#[command(about = "Eternal Terminal Production Server", long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "2022")]
    port: u16,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// Fixed passkey for all clients (32-byte hex string or 32-character ASCII string)
    /// If not provided, uses fixed test key (32 zero bytes)
    #[arg(short = 'k', long)]
    passkey: Option<String>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Run as daemon
    #[arg(short = 'd', long)]
    daemon: bool,
}

/// Connection type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionType {
    Terminal, // From etterminal-rs (has PTY)
    Client,   // From et-rs (user's local terminal)
}

/// Session state for a single client_id
struct Session {
    client_id: String,
    passkey: Vec<u8>,
    terminal_tx: Option<mpsc::Sender<Packet>>,
    client_tx: Option<mpsc::Sender<Packet>>,
}

/// Global server state
struct ServerState {
    sessions: RwLock<HashMap<String, Arc<RwLock<Session>>>>,
    fixed_passkey: Vec<u8>,
}

impl ServerState {
    fn new(fixed_passkey: Vec<u8>) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            fixed_passkey,
        }
    }

    /// Get or create a session for the given client_id
    fn get_or_create_session(&self, client_id: String, passkey: Vec<u8>) -> Arc<RwLock<Session>> {
        let mut sessions = self.sessions.write();
        sessions
            .entry(client_id.clone())
            .or_insert_with(|| {
                info!("Creating new session: {}", client_id);
                Arc::new(RwLock::new(Session {
                    client_id,
                    passkey,
                    terminal_tx: None,
                    client_tx: None,
                }))
            })
            .clone()
    }

    /// Remove a session
    #[allow(dead_code)]
    fn remove_session(&self, client_id: &str) {
        let mut sessions = self.sessions.write();
        if sessions.remove(client_id).is_some() {
            info!("Removed session: {}", client_id);
        }
    }
}

/// Parse passkey from string (hex or ASCII) to 32-byte key
fn parse_passkey(passkey: Option<&str>) -> Result<Vec<u8>> {
    match passkey {
        None => {
            // Default test key: 32 zero bytes
            Ok(vec![0u8; 32])
        }
        Some(key_str) => {
            // Try to parse as hex first (64 hex chars = 32 bytes)
            if key_str.len() == 64 && key_str.chars().all(|c| c.is_ascii_hexdigit()) {
                let mut key = Vec::with_capacity(32);
                for i in 0..32 {
                    let byte_str = &key_str[i * 2..i * 2 + 2];
                    let byte = u8::from_str_radix(byte_str, 16)
                        .map_err(|e| anyhow::anyhow!("Invalid hex passkey: {}", e))?;
                    key.push(byte);
                }
                Ok(key)
            } else if key_str.len() == 32 {
                // Treat as 32-character ASCII string (like C++ implementation)
                Ok(key_str.as_bytes().to_vec())
            } else {
                Err(anyhow::anyhow!(
                    "Passkey must be either 64 hex characters or 32 ASCII characters, got {} chars",
                    key_str.len()
                ))
            }
        }
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

    info!(
        "Starting Eternal Terminal Production Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Daemonize if requested
    if args.daemon {
        #[cfg(unix)]
        {
            use daemonize::Daemonize;
            info!("Running in daemon mode");

            let daemonize = Daemonize::new()
                .pid_file("/var/run/etserver.pid")
                .chown_pid_file(true)
                .working_directory("/tmp")
                .umask(0o027);

            match daemonize.start() {
                Ok(_) => info!("Daemon started successfully"),
                Err(e) => {
                    error!("Failed to daemonize: {}", e);
                    return Err(anyhow::anyhow!("Daemonization failed: {}", e));
                }
            }
        }
        #[cfg(not(unix))]
        {
            warn!("Daemon mode is only supported on Unix systems, continuing in foreground");
        }
    }

    info!("Listening on {}:{}", args.bind, args.port);

    // Parse passkey
    let fixed_passkey = parse_passkey(args.passkey.as_deref())?;
    info!(
        "Using {} passkey for all clients",
        if args.passkey.is_some() {
            "provided"
        } else {
            "default test"
        }
    );

    // Create server state
    let state = Arc::new(ServerState::new(fixed_passkey));

    // Create socket handler and listener
    let socket_handler = Arc::new(TcpSocketHandler::new());
    let mut endpoint = SocketEndpoint::default();
    endpoint.name = Some(args.bind.clone());
    endpoint.port = Some(args.port as i32);

    let mut listener = socket_handler.listen(&endpoint).await?;
    info!("Server ready, accepting connections...");

    // Accept connections in a loop
    loop {
        match listener.accept().await {
            Ok(socket) => {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(socket, state_clone).await {
                        error!("Connection handler error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

/// Handle a single connection (could be terminal or client)
async fn handle_connection(
    mut socket: Box<dyn AsyncSocket>,
    state: Arc<ServerState>,
) -> Result<()> {
    info!("New connection");

    // Read connect request
    let request: ConnectRequest = socket::read_proto(&mut *socket)
        .await
        .context("Failed to read connect request")?;

    let client_id = request
        .client_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Missing client_id"))?;
    let version = request.version.unwrap_or(0);

    info!("Connection request: client_id={}, version={}", client_id, version);

    // Validate protocol version
    if version != PROTOCOL_VERSION {
        warn!(
            "Protocol version mismatch: got {}, expected {}",
            version, PROTOCOL_VERSION
        );
        let mut response = ConnectResponse::default();
        response.status = Some(ConnectStatus::InvalidKey as i32);
        response.error = Some("Protocol version mismatch".to_string());
        socket::write_proto(&mut *socket, &response).await?;
        return Ok(());
    }

    // Get or create session
    let session = state.get_or_create_session(client_id.clone(), state.fixed_passkey.clone());

    // Send connect response
    let mut response = ConnectResponse::default();
    response.status = Some(ConnectStatus::NewClient as i32);
    socket::write_proto(&mut *socket, &response)
        .await
        .context("Failed to send connect response")?;

    info!("Connection accepted for client_id: {}", client_id);

    // Determine connection type
    // The first connection for a session is the terminal, the second is the client
    let conn_type = {
        let session_guard = session.read();
        if session_guard.terminal_tx.is_none() {
            ConnectionType::Terminal
        } else {
            ConnectionType::Client
        }
    };

    info!("Connection type: {:?}", conn_type);

    // Handle the connection based on type
    match conn_type {
        ConnectionType::Terminal => {
            handle_terminal_connection(socket, session, &state.fixed_passkey).await
        }
        ConnectionType::Client => {
            handle_client_connection(socket, session, &state.fixed_passkey).await
        }
    }
}

/// Handle terminal connection (from etterminal-rs)
async fn handle_terminal_connection(
    mut socket: Box<dyn AsyncSocket>,
    session: Arc<RwLock<Session>>,
    key: &[u8],
) -> Result<()> {
    let client_id = session.read().client_id.clone();
    info!("Terminal connected: {}", client_id);

    // Create channels for this connection
    // terminal_rx receives packets to send to terminal
    // terminal_tx sends packets received from terminal to client
    let (to_terminal_tx, mut to_terminal_rx) = mpsc::channel::<Packet>(100);

    // Store the terminal sender in the session
    {
        let mut session_guard = session.write();
        session_guard.terminal_tx = Some(to_terminal_tx);
    }

    // Create crypto handlers
    let mut reader_crypto = CryptoHandler::new(key, CLIENT_SERVER_NONCE_MSB)
        .context("Failed to create reader crypto")?;
    let writer_crypto = CryptoHandler::new(key, SERVER_CLIENT_NONCE_MSB)
        .context("Failed to create writer crypto")?;

    // Run bidirectional loop
    let mut buffer = BytesMut::with_capacity(8192);

    loop {
        tokio::select! {
            // Read from socket (terminal → server → client)
            result = read_packet(&mut socket, &mut reader_crypto, &mut buffer) => {
                match result {
                    Ok(packet) => {
                        debug!("Terminal → Server: {} bytes", packet.payload().len());
                        // Forward to client
                        let client_tx = {
                            let session_guard = session.read();
                            session_guard.client_tx.clone()
                        };
                        if let Some(client_tx) = client_tx {
                            if client_tx.send(packet).await.is_err() {
                                info!("Client disconnected, terminal waiting");
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Terminal read error: {}", e);
                        break;
                    }
                }
            }

            // Write to socket (client → server → terminal)
            Some(packet) = to_terminal_rx.recv() => {
                debug!("Server → Terminal: {} bytes", packet.payload().len());
                if let Err(e) = write_packet(&mut socket, &writer_crypto, &packet).await {
                    debug!("Terminal write error: {}", e);
                    break;
                }
            }
        }
    }

    // Clean up session
    {
        let mut session_guard = session.write();
        session_guard.terminal_tx = None;
        info!("Terminal disconnected: {}", client_id);
    }

    Ok(())
}

/// Handle client connection (from et-rs)
async fn handle_client_connection(
    mut socket: Box<dyn AsyncSocket>,
    session: Arc<RwLock<Session>>,
    key: &[u8],
) -> Result<()> {
    let client_id = session.read().client_id.clone();
    info!("Client connected: {}", client_id);

    // Create channels for this connection
    // client_rx receives packets to send to client
    // client_tx sends packets received from client to terminal
    let (to_client_tx, mut to_client_rx) = mpsc::channel::<Packet>(100);

    // Store the client sender in the session
    {
        let mut session_guard = session.write();
        session_guard.client_tx = Some(to_client_tx);
    }

    // Create crypto handlers
    let mut reader_crypto = CryptoHandler::new(key, CLIENT_SERVER_NONCE_MSB)
        .context("Failed to create reader crypto")?;
    let writer_crypto = CryptoHandler::new(key, SERVER_CLIENT_NONCE_MSB)
        .context("Failed to create writer crypto")?;

    // Run bidirectional loop
    let mut buffer = BytesMut::with_capacity(8192);

    loop {
        tokio::select! {
            // Read from socket (client → server → terminal)
            result = read_packet(&mut socket, &mut reader_crypto, &mut buffer) => {
                match result {
                    Ok(packet) => {
                        debug!("Client → Server: {} bytes", packet.payload().len());
                        // Forward to terminal
                        let terminal_tx = {
                            let session_guard = session.read();
                            session_guard.terminal_tx.clone()
                        };
                        if let Some(terminal_tx) = terminal_tx {
                            if terminal_tx.send(packet).await.is_err() {
                                info!("Terminal disconnected, client waiting");
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Client read error: {}", e);
                        break;
                    }
                }
            }

            // Write to socket (terminal → server → client)
            Some(packet) = to_client_rx.recv() => {
                debug!("Server → Client: {} bytes", packet.payload().len());
                if let Err(e) = write_packet(&mut socket, &writer_crypto, &packet).await {
                    debug!("Client write error: {}", e);
                    break;
                }
            }
        }
    }

    // Clean up session
    {
        let mut session_guard = session.write();
        session_guard.client_tx = None;
        info!("Client disconnected: {}", client_id);
    }

    Ok(())
}

/// Read a single packet from socket
async fn read_packet(
    socket: &mut Box<dyn AsyncSocket>,
    crypto: &mut CryptoHandler,
    buffer: &mut BytesMut,
) -> Result<Packet> {
    // Read packet length
    let mut len_buf = [0u8; 4];
    socket
        .read_exact(&mut len_buf)
        .await
        .context("Failed to read packet length")?;
    let length = u32::from_be_bytes(len_buf) as usize;

    if length == 0 || length > 10 * 1024 * 1024 {
        anyhow::bail!("Invalid packet length: {}", length);
    }

    // Read packet data
    buffer.clear();
    buffer.resize(length, 0);
    socket
        .read_exact(&mut buffer[..])
        .await
        .context("Failed to read packet data")?;

    // Deserialize and decrypt
    let mut packet = Packet::from_bytes(&buffer).context("Failed to deserialize packet")?;
    packet.decrypt(crypto).context("Failed to decrypt packet")?;

    Ok(packet)
}

/// Write a single packet to socket
async fn write_packet(
    socket: &mut Box<dyn AsyncSocket>,
    crypto: &CryptoHandler,
    packet: &Packet,
) -> Result<()> {
    // Encrypt packet
    let mut encrypted = packet.clone();
    encrypted
        .encrypt(crypto)
        .context("Failed to encrypt packet")?;

    // Serialize and write
    let serialized = encrypted.to_bytes();
    let header = (serialized.len() as u32).to_be_bytes();

    socket
        .write_all(&header)
        .await
        .context("Failed to write packet header")?;
    socket
        .write_all(&serialized)
        .await
        .context("Failed to write packet data")?;

    Ok(())
}
