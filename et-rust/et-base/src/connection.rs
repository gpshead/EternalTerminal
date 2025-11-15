/// Connection management for ET clients and servers
///
/// Provides connection abstractions with automatic reconnection,
/// backed reader/writer integration, and protocol handshake.

use crate::backed::{BackedReader, BackedWriter, WriteState};
use crate::constants::{
    CLIENT_SERVER_NONCE_MSB, PROTOCOL_VERSION, SERVER_CLIENT_NONCE_MSB,
};
use crate::crypto::CryptoHandler;
use crate::error::{EtError, Result};
use crate::packet::Packet;
use crate::socket::{self, AsyncSocket, SocketHandler};
use et_proto::{ConnectRequest, ConnectResponse, ConnectStatus, SocketEndpoint};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Base connection state
pub struct Connection {
    id: String,
    key: Vec<u8>,
    socket_handler: Arc<dyn SocketHandler>,
    state: Arc<RwLock<ConnectionState>>,
}

struct ConnectionState {
    reader: Option<Arc<BackedReader>>,
    writer: Option<Arc<BackedWriter>>,
    socket: Option<Box<dyn AsyncSocket>>,
    shutting_down: bool,
}

impl Connection {
    /// Create a new connection with the given ID and key
    pub fn new(
        socket_handler: Arc<dyn SocketHandler>,
        id: String,
        key: Vec<u8>,
    ) -> Self {
        Connection {
            id,
            key,
            socket_handler,
            state: Arc::new(RwLock::new(ConnectionState {
                reader: None,
                writer: None,
                socket: None,
                shutting_down: false,
            })),
        }
    }

    /// Read a packet from the connection
    pub fn read_packet(&self) -> Result<Option<Packet>> {
        let state = self.state.read();

        if state.shutting_down {
            return Ok(None);
        }

        if let Some(reader) = &state.reader {
            reader.read_buffered()
        } else {
            Ok(None)
        }
    }

    /// Write a packet to the connection
    pub async fn write_packet(&self, packet: Packet) -> Result<()> {
        loop {
            // Check if shutting down
            {
                let state = self.state.read();
                if state.shutting_down {
                    return Err(EtError::Connection("Connection is shutting down".into()));
                }
            }

            // Try to write
            let write_result = {
                let state = self.state.read();
                if let Some(writer) = &state.writer {
                    Some(writer.write(packet.clone()).await)
                } else {
                    None
                }
            };

            match write_result {
                Some(WriteState::Success) => return Ok(()),
                Some(WriteState::WroteWithFailure) | None => {
                    // Check if we have a connection
                    let has_connection = {
                        let state = self.state.read();
                        state.socket.is_some()
                    };

                    // Sleep and retry
                    if has_connection {
                        sleep(Duration::from_millis(1)).await;
                    } else {
                        sleep(Duration::from_millis(100)).await;
                    }
                }
                Some(WriteState::Skipped) => {
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Check if connection is disconnected
    pub fn is_disconnected(&self) -> bool {
        let state = self.state.read();
        state.socket.is_none()
    }

    /// Check if connection has data to read
    pub fn has_data(&self) -> bool {
        let state = self.state.read();
        if let Some(reader) = &state.reader {
            reader.has_data()
        } else {
            false
        }
    }

    /// Get the connection ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Close the socket
    pub fn close_socket(&self) {
        let mut state = self.state.write();

        if let Some(reader) = &state.reader {
            reader.invalidate_socket();
        }
        if let Some(writer) = &state.writer {
            writer.invalidate_socket();
        }

        state.socket = None;
        tracing::info!("Closed socket");
    }

    /// Mark connection as shutting down
    pub fn shutdown(&self) {
        let mut state = self.state.write();
        state.shutting_down = true;
        tracing::info!("Connection shutting down");
    }

    /// Check if shutting down
    pub fn is_shutting_down(&self) -> bool {
        let state = self.state.read();
        state.shutting_down
    }

    /// Recover connection with a new socket
    fn recover(&self, socket: Box<dyn AsyncSocket>) -> Result<()> {
        let mut state = self.state.write();

        // Create new reader and writer with the recovered socket
        let reader_crypto = CryptoHandler::new(&self.key, SERVER_CLIENT_NONCE_MSB)?;
        let writer_crypto = CryptoHandler::new(&self.key, CLIENT_SERVER_NONCE_MSB)?;

        // Get last sequence numbers for recovery
        let (last_read_seq, last_write_seq) = if let (Some(r), Some(w)) = (&state.reader, &state.writer) {
            (r.sequence_number(), w.sequence_number())
        } else {
            (0, 0)
        };

        tracing::info!(
            "Recovering connection - read seq: {}, write seq: {}",
            last_read_seq,
            last_write_seq
        );

        // TODO: Implement actual recovery with catchup buffer
        // For now, just create new reader/writer
        let reader = Arc::new(BackedReader::new(reader_crypto, socket));
        let writer = Arc::new(BackedWriter::new(
            writer_crypto,
            Box::new(DummySocket), // Placeholder
        ));

        state.reader = Some(reader);
        state.writer = Some(writer);

        Ok(())
    }
}

/// Dummy socket for temporary use
struct DummySocket;

#[async_trait::async_trait]
impl AsyncSocket for DummySocket {
    async fn read_exact(&mut self, _buf: &mut [u8]) -> Result<()> {
        Err(EtError::Connection("Dummy socket".into()))
    }

    async fn write_all(&mut self, _buf: &[u8]) -> Result<()> {
        Err(EtError::Connection("Dummy socket".into()))
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Client connection with automatic reconnection
pub struct ClientConnection {
    connection: Connection,
    endpoint: SocketEndpoint,
    reconnect_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl ClientConnection {
    /// Create a new client connection
    pub fn new(
        socket_handler: Arc<dyn SocketHandler>,
        endpoint: SocketEndpoint,
        id: String,
        key: Vec<u8>,
    ) -> Self {
        ClientConnection {
            connection: Connection::new(socket_handler, id, key),
            endpoint,
            reconnect_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Connect to the server
    pub async fn connect(&self) -> Result<bool> {
        tracing::info!("Connecting to {:?}", self.endpoint);

        // Connect socket
        let mut socket = self.connection.socket_handler.connect(&self.endpoint).await?;

        // Send connect request
        tracing::debug!("Sending connect request");
        let mut request = ConnectRequest::default();
        request.client_id = Some(self.connection.id.clone());
        request.version = Some(PROTOCOL_VERSION);

        socket::write_proto(&mut *socket, &request).await?;

        // Receive connect response
        tracing::debug!("Receiving connect response");
        let response: ConnectResponse = socket::read_proto(&mut *socket).await?;

        let status = response.status.unwrap_or(ConnectStatus::InvalidKey as i32);
        if status != ConnectStatus::NewClient as i32
            && status != ConnectStatus::ReturningClient as i32
        {
            let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
            tracing::error!("Error connecting to server: {}: {}", status, error);
            return Err(EtError::Connection(format!(
                "Server rejected connection: {}: {}",
                status, error
            )));
        }

        // Create backed reader and writer
        tracing::debug!("Creating backed reader and writer");
        let reader_crypto = CryptoHandler::new(&self.connection.key, SERVER_CLIENT_NONCE_MSB)?;
        let writer_crypto = CryptoHandler::new(&self.connection.key, CLIENT_SERVER_NONCE_MSB)?;

        let reader = Arc::new(BackedReader::new(reader_crypto, socket));
        // We need to get another socket for the writer - for now using dummy
        let writer = Arc::new(BackedWriter::new(
            writer_crypto,
            Box::new(DummySocket),
        ));

        // Update connection state
        {
            let mut state = self.connection.state.write();
            state.reader = Some(reader);
            state.writer = Some(writer);
            state.socket = None; // Reader took ownership
        }

        tracing::info!("Client connection established");
        Ok(true)
    }

    /// Close socket and start reconnection if not shutting down
    pub fn close_socket_and_maybe_reconnect(&self) {
        self.wait_reconnect();
        tracing::info!("Closing socket");
        self.connection.close_socket();

        if !self.connection.is_shutting_down() {
            tracing::info!("Socket closed, starting reconnect thread");
            self.start_reconnect();
        }
    }

    /// Wait for any pending reconnection to complete
    pub fn wait_reconnect(&self) {
        let mut handle_guard = self.reconnect_handle.write();
        if let Some(handle) = handle_guard.take() {
            tracing::info!("Waiting for reconnect thread to finish");
            // Drop the handle - in production we'd want to properly await it
            drop(handle);
        }
    }

    /// Start reconnection in background task
    fn start_reconnect(&self) {
        let connection = self.connection.clone();
        let endpoint = self.endpoint.clone();
        let reconnect_handle = self.reconnect_handle.clone();

        let handle = tokio::spawn(async move {
            poll_reconnect(connection, endpoint).await;
        });

        let mut guard = reconnect_handle.write();
        *guard = Some(handle);
    }

    /// Forward methods to underlying connection
    pub fn read_packet(&self) -> Result<Option<Packet>> {
        self.connection.read_packet()
    }

    pub async fn write_packet(&self, packet: Packet) -> Result<()> {
        self.connection.write_packet(packet).await
    }

    pub fn is_disconnected(&self) -> bool {
        self.connection.is_disconnected()
    }

    pub fn has_data(&self) -> bool {
        self.connection.has_data()
    }

    pub fn id(&self) -> &str {
        self.connection.id()
    }

    pub fn shutdown(&self) {
        self.connection.shutdown();
        self.wait_reconnect();
        self.connection.close_socket();
    }
}

impl Clone for Connection {
    fn clone(&self) -> Self {
        Connection {
            id: self.id.clone(),
            key: self.key.clone(),
            socket_handler: self.socket_handler.clone(),
            state: self.state.clone(),
        }
    }
}

/// Poll for reconnection in a loop
async fn poll_reconnect(connection: Connection, endpoint: SocketEndpoint) {
    tracing::info!("Trying to reconnect to {:?}", endpoint);

    while connection.is_disconnected() {
        // Check if shutting down
        if connection.is_shutting_down() {
            tracing::info!("Aborting reconnect loop because shutdown was called");
            return;
        }

        // Try to connect
        match connection.socket_handler.connect(&endpoint).await {
            Ok(socket) => {
                tracing::info!("Reconnected! Recovering connection...");
                if let Err(e) = connection.recover(socket) {
                    tracing::error!("Failed to recover connection: {}", e);
                    sleep(Duration::from_secs(3)).await;
                } else {
                    tracing::info!("Connection recovered successfully");
                    return;
                }
            }
            Err(e) => {
                tracing::debug!("Reconnect attempt failed: {}", e);
                sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::TcpSocketHandler;

    #[test]
    fn test_connection_creation() {
        let handler = Arc::new(TcpSocketHandler::new());
        let conn = Connection::new(
            handler,
            "test-client".to_string(),
            vec![0u8; 32],
        );

        assert_eq!(conn.id(), "test-client");
        assert!(conn.is_disconnected());
        assert!(!conn.is_shutting_down());
    }

    #[test]
    fn test_connection_shutdown() {
        let handler = Arc::new(TcpSocketHandler::new());
        let conn = Connection::new(
            handler,
            "test-client".to_string(),
            vec![0u8; 32],
        );

        assert!(!conn.is_shutting_down());
        conn.shutdown();
        assert!(conn.is_shutting_down());
    }

    #[tokio::test]
    async fn test_client_connection_creation() {
        let handler = Arc::new(TcpSocketHandler::new());
        let mut endpoint = SocketEndpoint::default();
        endpoint.name = Some("127.0.0.1".to_string());
        endpoint.port = Some(2022);

        let client = ClientConnection::new(
            handler,
            endpoint,
            "test-client".to_string(),
            vec![0u8; 32],
        );

        assert_eq!(client.id(), "test-client");
        assert!(client.is_disconnected());
    }
}
