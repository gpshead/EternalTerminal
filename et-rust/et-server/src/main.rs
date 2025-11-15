/// Minimal ET Server for interoperability testing
///
/// This server:
/// - Listens on a TCP port
/// - Accepts ET client connections
/// - Handles protocol handshake
/// - Echoes packets back to clients (for testing)

use anyhow::Result;
use clap::Parser;
use et_base::{
    socket::{self, AsyncSocket, SocketHandler, TcpSocketHandler},
    CryptoHandler, Packet, CRYPTO_KEY_BYTES, CLIENT_SERVER_NONCE_MSB, PROTOCOL_VERSION,
    SERVER_CLIENT_NONCE_MSB,
};
use et_proto::{ConnectRequest, ConnectResponse, ConnectStatus, SocketEndpoint};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(name = "etserver-rs")]
#[command(about = "Eternal Terminal Server (Rust)", long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "2022")]
    port: u16,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

/// Server state tracking connected clients
struct ServerState {
    clients: HashMap<String, ClientInfo>,
}

struct ClientInfo {
    key: Vec<u8>,
    #[allow(dead_code)]
    last_seen: std::time::Instant,
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

    info!("Starting Eternal Terminal Server (Rust) v{}", env!("CARGO_PKG_VERSION"));
    info!("Listening on {}:{}", args.bind, args.port);

    // Create server state
    let state = Arc::new(Mutex::new(ServerState {
        clients: HashMap::new(),
    }));

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
                    if let Err(e) = handle_client(socket, state_clone).await {
                        error!("Client handler error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

/// Handle a single client connection
async fn handle_client(
    mut socket: Box<dyn AsyncSocket>,
    state: Arc<Mutex<ServerState>>,
) -> Result<()> {
    info!("New client connection");

    // Read connect request
    let request: ConnectRequest = socket::read_proto(&mut *socket).await?;

    let client_id = request
        .client_id
        .ok_or_else(|| anyhow::anyhow!("Missing client ID"))?;
    let client_version = request.version.unwrap_or(0);

    info!("Client {} connecting (version {})", client_id, client_version);

    // Check protocol version
    if client_version != PROTOCOL_VERSION {
        warn!(
            "Protocol version mismatch: client={}, server={}",
            client_version, PROTOCOL_VERSION
        );
        let mut response = ConnectResponse::default();
        response.status = Some(ConnectStatus::MismatchedProtocol as i32);
        response.error = Some(format!(
            "Version mismatch: server={}, client={}",
            PROTOCOL_VERSION, client_version
        ));
        socket::write_proto(&mut *socket, &response).await?;
        return Ok(());
    }

    // Check if client exists or is new
    let (status, key) = {
        let mut state_guard = state.lock().await;
        if let Some(client_info) = state_guard.clients.get(&client_id) {
            info!("Returning client: {}", client_id);
            (ConnectStatus::ReturningClient, client_info.key.clone())
        } else {
            info!("New client: {}", client_id);
            // Generate a new key for this client
            let key = generate_key();
            state_guard.clients.insert(
                client_id.clone(),
                ClientInfo {
                    key: key.clone(),
                    last_seen: std::time::Instant::now(),
                },
            );
            (ConnectStatus::NewClient, key)
        }
    };

    // Send connect response
    let mut response = ConnectResponse::default();
    response.status = Some(status as i32);
    socket::write_proto(&mut *socket, &response).await?;

    info!("Client {} successfully connected", client_id);

    // Create crypto handlers for this connection
    let reader_crypto = CryptoHandler::new(&key, CLIENT_SERVER_NONCE_MSB)?;
    let writer_crypto = CryptoHandler::new(&key, SERVER_CLIENT_NONCE_MSB)?;

    // Enter packet echo loop
    info!("Entering packet echo loop for client {}", client_id);
    let result = packet_echo_loop(&mut socket, reader_crypto, writer_crypto).await;

    match result {
        Ok(_) => info!("Client {} disconnected cleanly", client_id),
        Err(e) => warn!("Client {} connection error: {}", client_id, e),
    }

    Ok(())
}

/// Echo packets back to the client (for testing)
async fn packet_echo_loop(
    socket: &mut Box<dyn AsyncSocket>,
    reader_crypto: CryptoHandler,
    writer_crypto: CryptoHandler,
) -> Result<()> {
    let mut packet_count = 0;

    loop {
        // Read packet with 4-byte length header
        let mut len_buf = [0u8; 4];
        match socket.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(_) => {
                // Connection closed
                info!("Connection closed (after {} packets)", packet_count);
                return Ok(());
            }
        }

        let length = u32::from_be_bytes(len_buf) as usize;

        if length == 0 {
            warn!("Received zero-length packet");
            continue;
        }

        if length > 10 * 1024 * 1024 {
            error!("Packet too large: {} bytes", length);
            return Err(anyhow::anyhow!("Packet too large"));
        }

        // Read packet data
        let mut data = vec![0u8; length];
        socket.read_exact(&mut data).await?;

        // Deserialize and decrypt
        let mut packet = Packet::from_bytes(&data)?;
        packet.decrypt(&reader_crypto)?;

        packet_count += 1;
        info!(
            "Received packet {}: header={}, payload_len={}",
            packet_count,
            packet.header(),
            packet.payload().len()
        );

        // Echo packet back
        let mut echo_packet = packet.clone();
        echo_packet.encrypt(&writer_crypto)?;

        let serialized = echo_packet.to_bytes();
        let header = (serialized.len() as u32).to_be_bytes();

        socket.write_all(&header).await?;
        socket.write_all(&serialized).await?;

        info!("Echoed packet {} back to client", packet_count);
    }
}

/// Generate a key for a client
///
/// For interop testing, we use a fixed test key.
/// In production, this would generate a random key and securely share it.
fn generate_key() -> Vec<u8> {
    // Fixed test key for interop testing
    // In production, use: rand::thread_rng().gen::<[u8; CRYPTO_KEY_BYTES]>().to_vec()
    vec![0u8; CRYPTO_KEY_BYTES]
}
