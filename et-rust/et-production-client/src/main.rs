/// Production ET Client
///
/// This is the production client binary that users interact with.
/// It handles the full connection flow:
/// 1. Parse command-line arguments (user@host[:port])
/// 2. Generate a secure passkey
/// 3. Use SSH to spawn etterminal-rs on the remote server
/// 4. Connect to the ET server
/// 5. Enter local terminal raw mode
/// 6. Bridge local terminal ↔ ET connection

use anyhow::{Context, Result};
use bytes::BytesMut;
use clap::Parser;
use et_base::{
    socket::{self, AsyncSocket, SocketHandler, TcpSocketHandler},
    CryptoHandler, Packet, CLIENT_SERVER_NONCE_MSB, PROTOCOL_VERSION, SERVER_CLIENT_NONCE_MSB,
};
use et_proto::{ConnectRequest, ConnectResponse, ConnectStatus, SocketEndpoint};
use et_terminal::{parse_ssh_target, RawModeGuard, SshClient, SshConfig, Config};
use rand::Rng;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "et-rs")]
#[command(about = "Eternal Terminal - Production Client", long_about = None)]
struct Args {
    /// Target: [user@]host[:port]
    target: String,

    /// ET server port (overrides port in target)
    #[arg(short = 'p', long, default_value = "2022")]
    port: u16,

    /// SSH port for remote connection
    #[arg(long, default_value = "22")]
    ssh_port: u16,

    /// SSH identity file (private key)
    #[arg(short = 'i', long)]
    identity_file: Option<PathBuf>,

    /// Command to run on remote server (if not interactive shell)
    #[arg(short = 'c', long)]
    command: Option<String>,

    /// ET server address on remote (usually localhost)
    #[arg(long, default_value = "127.0.0.1")]
    et_server: String,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Local port forward: -L local_port:remote_host:remote_port
    #[arg(short = 'L', long = "local-forward", value_name = "SPEC")]
    local_forwards: Vec<String>,

    /// Remote port forward: -R remote_port:local_host:local_port
    #[arg(short = 'R', long = "remote-forward", value_name = "SPEC")]
    remote_forwards: Vec<String>,

    /// Jumphost: user@host for intermediate SSH hop
    #[arg(short = 'J', long)]
    jumphost: Option<String>,

    /// Disable config file loading
    #[arg(long)]
    no_config: bool,
}

/// Generate a secure random 32-byte passkey
fn generate_passkey() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut key = vec![0u8; 32];
    rng.fill(&mut key[..]);
    key
}

/// Convert passkey bytes to hex string for passing via command line
fn passkey_to_hex(key: &[u8]) -> String {
    key.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
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

    info!("ET Production Client v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration file (unless disabled)
    let config = if args.no_config {
        Config::default()
    } else {
        Config::load().unwrap_or_else(|e| {
            debug!("Failed to load config: {}, using defaults", e);
            Config::default()
        })
    };

    // Parse target: [user@]host[:port]
    let (user, host, ssh_port) = parse_ssh_target(&args.target)
        .context("Failed to parse target")?;

    // Get host-specific settings from config
    let host_settings = config.get_host_config(&host);
    debug!("Host settings: {:?}", host_settings);

    // Determine SSH port: CLI arg > target port > config > default
    let ssh_port = if ssh_port != 22 {
        ssh_port
    } else if args.ssh_port != 22 {
        args.ssh_port
    } else {
        host_settings.ssh_port
    };

    // Determine user: target user > config user > default
    let user = if !user.is_empty() {
        user
    } else if let Some(ref config_user) = host_settings.user {
        config_user.clone()
    } else {
        user
    };

    // Determine identity file: CLI arg > config
    let identity_file = args.identity_file
        .or(host_settings.identity_file);

    // Determine jumphost: CLI arg > config
    let jumphost = args.jumphost
        .or(host_settings.jumphost);

    if let Some(ref jh) = jumphost {
        info!("Using jumphost: {}", jh);
    }

    info!("Connecting to {}@{} (SSH port {})", user, host, ssh_port);

    // Generate client ID and passkey
    let client_id = Uuid::new_v4().to_string();
    let passkey = generate_passkey();
    let passkey_hex = passkey_to_hex(&passkey);

    debug!("Client ID: {}", client_id);
    debug!("Passkey: {} bytes", passkey.len());

    // Build etterminal command to run on remote server
    let mut etterminal_cmd = format!(
        "etterminal-rs --client-id {} --passkey {} --server {} --port {}",
        client_id, passkey_hex, args.et_server, args.port
    );

    // Get terminal size for initial PTY size
    if let Ok(size) = et_terminal::TerminalSize::from_stdin() {
        etterminal_cmd.push_str(&format!(
            " --rows {} --cols {}",
            size.rows, size.cols
        ));
    }

    // Add command if specified
    if let Some(ref cmd) = args.command {
        etterminal_cmd.push_str(&format!(" --command '{}'", cmd.replace("'", "'\\''")));
    }

    debug!("Remote command: {}", etterminal_cmd);

    // TODO: Implement port forwarding
    if !args.local_forwards.is_empty() {
        info!("Local port forwards requested: {:?}", args.local_forwards);
        warn!("Port forwarding not yet implemented in Phase 6");
    }
    if !args.remote_forwards.is_empty() {
        info!("Remote port forwards requested: {:?}", args.remote_forwards);
        warn!("Port forwarding not yet implemented in Phase 6");
    }

    // TODO: Implement jumphost support
    // For now, we connect directly. Full jumphost support requires:
    // 1. SSH to jumphost
    // 2. From jumphost, SSH to final destination
    // 3. Spawn etterminal on final destination
    if jumphost.is_some() {
        warn!("Jumphost support not yet fully implemented in Phase 6");
        warn!("Connecting directly to target instead");
    }

    // Connect via SSH and spawn etterminal
    info!("Spawning remote terminal via SSH...");
    let ssh_config = SshConfig {
        host: host.clone(),
        port: ssh_port,
        user: user.clone(),
        identity_file,
        password: None,
    };

    let ssh_client = SshClient::connect(ssh_config)
        .context("Failed to connect via SSH")?;

    info!("SSH connection established");

    // Execute etterminal in the background on remote server
    // We don't wait for it to finish - it will run until we disconnect
    let channel = ssh_client
        .execute_interactive(&etterminal_cmd)
        .context("Failed to spawn etterminal on remote server")?;

    info!("Remote terminal spawned");

    // Give etterminal a moment to start and connect to the ET server
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Connect to ET server
    info!("Connecting to ET server at {}:{}...", host, args.port);
    let socket = connect_to_server(&host, args.port, &client_id).await
        .context("Failed to connect to ET server")?;

    info!("Connected to ET server");

    // Enter raw mode for local terminal
    let raw_mode = RawModeGuard::enable(io::stdin().as_raw_fd())
        .context("Failed to enter raw mode")?;

    info!("Entering interactive mode (press Ctrl+D to exit)");

    // Set up signal handling for terminal resize
    let signals = Signals::new(&[SIGWINCH])
        .context("Failed to set up signal handling")?;
    let signal_handle = signals.handle();

    let terminal_size = Arc::new(Mutex::new(et_terminal::TerminalSize::from_stdin().ok()));

    let size_clone = terminal_size.clone();
    let signal_task = tokio::spawn(async move {
        handle_signals(signals, size_clone).await;
    });

    // Run bidirectional bridge: Local Terminal ↔ ET Connection
    let result = bridge_terminal_connection(socket, &passkey, terminal_size).await;

    // Clean up
    signal_handle.close();
    signal_task.abort();
    drop(raw_mode); // Restore terminal mode
    drop(channel); // Close SSH channel

    match result {
        Ok(_) => {
            info!("Session ended normally");
            Ok(())
        }
        Err(e) => {
            error!("Session ended with error: {}", e);
            Err(e)
        }
    }
}

/// Connect to ET server and perform handshake
async fn connect_to_server(host: &str, port: u16, client_id: &str) -> Result<Box<dyn AsyncSocket>> {
    let socket_handler = Arc::new(TcpSocketHandler::new());
    let mut endpoint = SocketEndpoint::default();
    endpoint.name = Some(host.to_string());
    endpoint.port = Some(port as i32);

    // Connect to server
    debug!("Connecting to {}:{}", host, port);
    let mut socket = socket_handler
        .connect(&endpoint)
        .await
        .context("Failed to connect to ET server")?;

    // Send connect request
    debug!("Sending connect request");
    let mut request = ConnectRequest::default();
    request.client_id = Some(client_id.to_string());
    request.version = Some(PROTOCOL_VERSION);

    socket::write_proto(&mut *socket, &request)
        .await
        .context("Failed to send connect request")?;

    // Receive connect response
    debug!("Waiting for connect response");
    let response: ConnectResponse = socket::read_proto(&mut *socket)
        .await
        .context("Failed to receive connect response")?;

    let status = response.status.unwrap_or(ConnectStatus::InvalidKey as i32);
    if status != ConnectStatus::NewClient as i32
        && status != ConnectStatus::ReturningClient as i32
    {
        let error = response
            .error
            .unwrap_or_else(|| "Unknown error".to_string());
        anyhow::bail!(
            "Server rejected connection: status={}, error={}",
            status,
            error
        );
    }

    let status_str = if status == ConnectStatus::NewClient as i32 {
        "NEW_CLIENT"
    } else {
        "RETURNING_CLIENT"
    };
    info!("Connection accepted: {}", status_str);

    Ok(socket)
}

/// Bridge local terminal and ET connection bidirectionally
async fn bridge_terminal_connection(
    mut socket: Box<dyn AsyncSocket>,
    key: &[u8],
    _terminal_size: Arc<Mutex<Option<et_terminal::TerminalSize>>>,
) -> Result<()> {
    // Create crypto handlers
    let writer_crypto = CryptoHandler::new(key, CLIENT_SERVER_NONCE_MSB)
        .context("Failed to create writer crypto")?;
    let reader_crypto = CryptoHandler::new(key, SERVER_CLIENT_NONCE_MSB)
        .context("Failed to create reader crypto")?;

    let writer_crypto = Arc::new(Mutex::new(writer_crypto));
    let reader_crypto = Arc::new(Mutex::new(reader_crypto));

    // Create buffer for socket
    let mut socket_buffer = BytesMut::with_capacity(8192);

    // Set stdin to non-blocking mode
    let stdin_fd = io::stdin().as_raw_fd();
    let flags = nix::fcntl::fcntl(stdin_fd, nix::fcntl::FcntlArg::F_GETFL)
        .context("Failed to get stdin flags")?;
    let mut flags = nix::fcntl::OFlag::from_bits_truncate(flags);
    flags.insert(nix::fcntl::OFlag::O_NONBLOCK);
    nix::fcntl::fcntl(stdin_fd, nix::fcntl::FcntlArg::F_SETFL(flags))
        .context("Failed to set stdin non-blocking")?;

    loop {
        tokio::select! {
            // Local stdin → Socket: Read from stdin, send to ET server
            result = tokio::task::spawn_blocking(|| {
                let mut stdin = io::stdin();
                let mut buffer = vec![0u8; 8192];
                match stdin.read(&mut buffer) {
                    Ok(n) => Ok((n, buffer)),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok((0, buffer)),
                    Err(e) => Err(e),
                }
            }) => {
                match result {
                    Ok(Ok((0, _buf))) => {
                        // No data available
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Ok(Ok((n, buf))) => {
                        debug!("Read {} bytes from stdin", n);

                        // Create and encrypt packet
                        let packet = Packet::new(1, buf[..n].to_vec());
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

                        debug!("Sent {} bytes to server", n);
                    }
                    Ok(Err(e)) => {
                        if e.kind() != io::ErrorKind::Interrupted {
                            warn!("Stdin read error: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Task join error: {}", e);
                        break;
                    }
                }
            },

            // Socket → Local stdout: Read from server, write to stdout
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

                        // Write payload to stdout
                        let payload = packet.payload();
                        if !payload.is_empty() {
                            io::stdout().write_all(payload)
                                .context("Failed to write to stdout")?;
                            io::stdout().flush()
                                .context("Failed to flush stdout")?;
                            debug!("Wrote {} bytes to stdout", payload.len());
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
        }
    }

    Ok(())
}

/// Handle signals (SIGWINCH for terminal resize)
async fn handle_signals(
    mut signals: Signals,
    terminal_size: Arc<Mutex<Option<et_terminal::TerminalSize>>>,
) {
    use futures::stream::StreamExt;

    while let Some(signal) = signals.next().await {
        match signal {
            SIGWINCH => {
                debug!("Received SIGWINCH");

                // Get current terminal size
                if let Ok(size) = et_terminal::TerminalSize::from_stdin() {
                    let mut size_guard = terminal_size.lock().await;
                    *size_guard = Some(size);
                    info!("Terminal resized: {}x{}", size.rows, size.cols);

                    // TODO: Send resize message to remote terminal
                    // This will require a new packet type in the protocol
                }
            }
            _ => {
                debug!("Received signal: {}", signal);
            }
        }
    }
}
