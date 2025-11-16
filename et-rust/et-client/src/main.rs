/// Minimal ET Client for interoperability testing
///
/// This client:
/// - Connects to an ET server
/// - Performs protocol handshake
/// - Sends test packets
/// - Validates echo responses

use anyhow::Result;
use clap::Parser;
use et_base::{
    socket::{self, AsyncSocket, SocketHandler, TcpSocketHandler},
    CryptoHandler, Packet, CLIENT_SERVER_NONCE_MSB, PROTOCOL_VERSION,
    SERVER_CLIENT_NONCE_MSB,
};
use et_proto::{ConnectRequest, ConnectResponse, ConnectStatus, SocketEndpoint};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "etclient-rs")]
#[command(about = "Eternal Terminal Client (Rust)", long_about = None)]
struct Args {
    /// Server hostname
    #[arg(short = 'H', long, default_value = "localhost")]
    host: String,

    /// Server port
    #[arg(short, long, default_value = "2022")]
    port: u16,

    /// Client ID (auto-generated if not provided)
    #[arg(short, long)]
    client_id: Option<String>,

    /// Number of test packets to send
    #[arg(short = 'n', long, default_value = "5")]
    num_packets: usize,

    /// Passkey (32-byte hex string or 32-character ASCII string)
    /// If not provided, uses fixed test key (32 zero bytes)
    #[arg(short = 'k', long)]
    passkey: Option<String>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
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

    // Generate or use provided client ID
    let client_id = args
        .client_id
        .unwrap_or_else(|| format!("rust-client-{}", Uuid::new_v4()));

    info!("Starting Eternal Terminal Client (Rust) v{}", env!("CARGO_PKG_VERSION"));
    info!("Connecting to {}:{}", args.host, args.port);
    info!("Client ID: {}", client_id);

    // Create socket handler and endpoint
    let socket_handler = Arc::new(TcpSocketHandler::new());
    let mut endpoint = SocketEndpoint::default();
    endpoint.name = Some(args.host.clone());
    endpoint.port = Some(args.port as i32);

    // Connect to server
    info!("Connecting to server...");
    let mut socket = socket_handler.connect(&endpoint).await?;
    info!("TCP connection established");

    // Send connect request
    info!("Sending connect request");
    let mut request = ConnectRequest::default();
    request.client_id = Some(client_id.clone());
    request.version = Some(PROTOCOL_VERSION);

    socket::write_proto(&mut *socket, &request).await?;

    // Receive connect response
    info!("Waiting for connect response");
    let response: ConnectResponse = socket::read_proto(&mut *socket).await?;

    let status = response.status.unwrap_or(ConnectStatus::InvalidKey as i32);
    if status != ConnectStatus::NewClient as i32
        && status != ConnectStatus::ReturningClient as i32
    {
        let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
        error!("Server rejected connection: status={}, error={}", status, error);
        return Err(anyhow::anyhow!("Connection rejected: {}", error));
    }

    let status_str = if status == ConnectStatus::NewClient as i32 {
        "NEW_CLIENT"
    } else {
        "RETURNING_CLIENT"
    };
    info!("Connection accepted: {}", status_str);

    // Parse passkey from command line or use default test key
    let key = parse_passkey(args.passkey.as_deref())?;
    info!("Using {} passkey", if args.passkey.is_some() { "provided" } else { "default test" });

    // Create crypto handlers
    info!("Setting up encryption");
    let reader_crypto = CryptoHandler::new(&key, SERVER_CLIENT_NONCE_MSB)?;
    let writer_crypto = CryptoHandler::new(&key, CLIENT_SERVER_NONCE_MSB)?;

    // Run packet test
    info!("Starting packet exchange test ({} packets)", args.num_packets);
    test_packet_exchange(
        &mut socket,
        reader_crypto,
        writer_crypto,
        args.num_packets,
    )
    .await?;

    info!("Test completed successfully!");
    Ok(())
}

/// Test packet exchange with the server
async fn test_packet_exchange(
    socket: &mut Box<dyn AsyncSocket>,
    reader_crypto: CryptoHandler,
    writer_crypto: CryptoHandler,
    num_packets: usize,
) -> Result<()> {
    for i in 1..=num_packets {
        // Create test packet
        let payload = format!("Test packet {}", i);
        let mut packet = Packet::new(i as u8, payload.as_bytes().to_vec());

        info!("Sending packet {}: '{}'", i, payload);

        // Encrypt and send
        packet.encrypt(&writer_crypto)?;
        let serialized = packet.to_bytes();
        let header = (serialized.len() as u32).to_be_bytes();

        socket.write_all(&header).await?;
        socket.write_all(&serialized).await?;

        // Read echo response
        info!("Waiting for echo response...");
        let mut len_buf = [0u8; 4];
        socket.read_exact(&mut len_buf).await?;
        let length = u32::from_be_bytes(len_buf) as usize;

        if length == 0 {
            error!("Received zero-length response");
            return Err(anyhow::anyhow!("Zero-length response"));
        }

        if length > 10 * 1024 * 1024 {
            error!("Response too large: {} bytes", length);
            return Err(anyhow::anyhow!("Response too large"));
        }

        let mut data = vec![0u8; length];
        socket.read_exact(&mut data).await?;

        // Decrypt response
        let mut response_packet = Packet::from_bytes(&data)?;
        response_packet.decrypt(&reader_crypto)?;

        info!(
            "Received echo: header={}, payload_len={}",
            response_packet.header(),
            response_packet.payload().len()
        );

        // Validate echo
        if response_packet.header() != packet.header() {
            error!(
                "Header mismatch: sent={}, received={}",
                packet.header(),
                response_packet.header()
            );
            return Err(anyhow::anyhow!("Header mismatch"));
        }

        let response_payload = String::from_utf8_lossy(response_packet.payload());
        info!("Echo validated: '{}'", response_payload);

        // Small delay between packets
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(())
}
