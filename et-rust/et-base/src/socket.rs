/// Socket abstractions for ET
///
/// Provides a trait-based abstraction over different socket types (TCP, Unix, Pipe)
/// similar to the C++ SocketHandler hierarchy.

use crate::error::{EtError, Result};
use crate::packet::Packet;
use async_trait::async_trait;
use et_proto::SocketEndpoint;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Maximum protobuf/packet message size (128MB)
pub const MAX_MESSAGE_SIZE: usize = 128 * 1024 * 1024;

/// Trait for socket operations
///
/// This mirrors the C++ SocketHandler abstract class
#[async_trait]
pub trait SocketHandler: Send + Sync {
    /// Connect to a remote endpoint
    async fn connect(&self, endpoint: &SocketEndpoint) -> Result<Box<dyn AsyncSocket>>;

    /// Listen on an endpoint and accept connections
    async fn listen(&self, endpoint: &SocketEndpoint) -> Result<Box<dyn AsyncListener>>;
}

/// Trait for an established socket connection
#[async_trait]
pub trait AsyncSocket: Send + Sync {
    /// Read exactly `count` bytes into buffer
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()>;

    /// Write all bytes from buffer
    async fn write_all(&mut self, buf: &[u8]) -> Result<()>;

    /// Shutdown the socket
    async fn shutdown(&mut self) -> Result<()>;

    /// Read a packet from the socket
    async fn read_packet(&mut self) -> Result<Packet> {
        // Read length prefix (8 bytes)
        let mut len_buf = [0u8; 8];
        self.read_exact(&mut len_buf).await?;
        let length = i64::from_le_bytes(len_buf);

        if length < 0 || length as usize > MAX_MESSAGE_SIZE {
            return Err(EtError::Protocol(format!(
                "Invalid packet size: {}",
                length
            )));
        }

        if length == 0 {
            return Err(EtError::ConnectionClosed);
        }

        // Read packet data
        let mut data = vec![0u8; length as usize];
        self.read_exact(&mut data).await?;

        Packet::from_bytes(&data)
    }

    /// Write a packet to the socket
    async fn write_packet(&mut self, packet: &Packet) -> Result<()> {
        let data = packet.to_bytes();
        let length = data.len() as i64;

        if length < 0 || length as usize > MAX_MESSAGE_SIZE {
            return Err(EtError::Protocol(format!(
                "Invalid packet size: {}",
                length
            )));
        }

        // Write length prefix
        self.write_all(&length.to_le_bytes()).await?;

        // Write packet data
        if length > 0 {
            self.write_all(&data).await?;
        }

        Ok(())
    }

}

/// Helper functions for reading/writing protobuf messages
/// These are standalone functions since they use generics
pub async fn read_proto<M: prost::Message + Default, S: AsyncSocket + ?Sized>(
    socket: &mut S,
) -> Result<M> {
    // Read length prefix
    let mut len_buf = [0u8; 8];
    socket.read_exact(&mut len_buf).await?;
    let length = i64::from_le_bytes(len_buf);

    if length < 0 || length as usize > MAX_MESSAGE_SIZE {
        return Err(EtError::Protocol(format!(
            "Invalid proto size: {}",
            length
        )));
    }

    if length == 0 {
        return Ok(M::default());
    }

    // Read message data
    let mut data = vec![0u8; length as usize];
    socket.read_exact(&mut data).await?;

    M::decode(&data[..]).map_err(|e| EtError::Serialization(e.to_string()))
}

/// Write a protobuf message to a socket
pub async fn write_proto<M: prost::Message, S: AsyncSocket + ?Sized>(
    socket: &mut S,
    msg: &M,
) -> Result<()> {
    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| EtError::Serialization(e.to_string()))?;

    let length = buf.len() as i64;
    if length < 0 || length as usize > MAX_MESSAGE_SIZE {
        return Err(EtError::Protocol(format!(
            "Invalid proto size: {}",
            length
        )));
    }

    // Write length prefix
    socket.write_all(&length.to_le_bytes()).await?;

    // Write message data
    if length > 0 {
        socket.write_all(&buf).await?;
    }

    Ok(())
}

/// Trait for a listening socket
#[async_trait]
pub trait AsyncListener: Send + Sync {
    /// Accept a new connection
    async fn accept(&mut self) -> Result<Box<dyn AsyncSocket>>;

    /// Stop listening (close the listener)
    async fn stop(&mut self) -> Result<()>;
}

/// TCP socket implementation
pub struct TcpSocket {
    stream: TcpStream,
}

impl TcpSocket {
    pub fn new(stream: TcpStream) -> Self {
        TcpSocket { stream }
    }
}

#[async_trait]
impl AsyncSocket for TcpSocket {
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.stream
            .read_exact(buf)
            .await
            .map(|_| ())
            .map_err(EtError::from)
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.stream
            .write_all(buf)
            .await
            .map_err(EtError::from)
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.stream
            .shutdown()
            .await
            .map_err(EtError::from)
    }
}

/// TCP listener implementation
pub struct TcpSocketListener {
    listener: TcpListener,
}

impl TcpSocketListener {
    pub fn new(listener: TcpListener) -> Self {
        TcpSocketListener { listener }
    }
}

#[async_trait]
impl AsyncListener for TcpSocketListener {
    async fn accept(&mut self) -> Result<Box<dyn AsyncSocket>> {
        let (stream, _addr) = self.listener.accept().await?;

        // Set TCP_NODELAY for low latency
        stream.set_nodelay(true)?;

        Ok(Box::new(TcpSocket::new(stream)))
    }

    async fn stop(&mut self) -> Result<()> {
        // Dropping the listener closes it
        Ok(())
    }
}

/// TCP socket handler
pub struct TcpSocketHandler;

impl TcpSocketHandler {
    pub fn new() -> Self {
        TcpSocketHandler
    }
}

impl Default for TcpSocketHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SocketHandler for TcpSocketHandler {
    async fn connect(&self, endpoint: &SocketEndpoint) -> Result<Box<dyn AsyncSocket>> {
        let name = endpoint.name.as_ref().ok_or_else(|| {
            EtError::Protocol("Endpoint missing name".to_string())
        })?;

        let port = endpoint.port.ok_or_else(|| {
            EtError::Protocol("Endpoint missing port".to_string())
        })?;

        let addr = format!("{}:{}", name, port);
        let stream = TcpStream::connect(&addr).await?;

        // Set TCP_NODELAY for low latency
        stream.set_nodelay(true)?;

        Ok(Box::new(TcpSocket::new(stream)))
    }

    async fn listen(&self, endpoint: &SocketEndpoint) -> Result<Box<dyn AsyncListener>> {
        let name = endpoint
            .name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("0.0.0.0");

        let port = endpoint.port.ok_or_else(|| {
            EtError::Protocol("Endpoint missing port".to_string())
        })?;

        let addr = format!("{}:{}", name, port);
        let listener = TcpListener::bind(&addr).await?;

        Ok(Box::new(TcpSocketListener::new(listener)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CryptoHandler, CRYPTO_KEY_BYTES};

    #[tokio::test]
    async fn test_tcp_socket_connect_and_listen() {
        let handler = TcpSocketHandler::new();

        // Create endpoint
        let mut endpoint = SocketEndpoint::default();
        endpoint.name = Some("127.0.0.1".to_string());
        endpoint.port = Some(0); // Let OS assign port

        // Start listener
        let _listener = handler.listen(&endpoint).await.unwrap();

        // In a real test, we'd spawn a task to accept and another to connect
        // For now, just verify we can create the listener
    }

    #[tokio::test]
    async fn test_packet_read_write() {
        use tokio::io::DuplexStream;

        // Create an in-memory duplex stream for testing
        let (client, server) = tokio::io::duplex(1024);

        // Wrap in our socket trait
        struct TestSocket(DuplexStream);

        #[async_trait]
        impl AsyncSocket for TestSocket {
            async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
                self.0.read_exact(buf).await.map(|_| ()).map_err(EtError::from)
            }

            async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
                self.0.write_all(buf).await.map_err(EtError::from)
            }

            async fn shutdown(&mut self) -> Result<()> {
                Ok(())
            }
        }

        let mut client_socket = TestSocket(client);
        let mut server_socket = TestSocket(server);

        // Create and send a packet from client to server
        let packet = Packet::new(42, b"Hello, Server!".to_vec());

        // Spawn task to write
        let write_task = tokio::spawn(async move {
            client_socket.write_packet(&packet).await.unwrap();
            client_socket
        });

        // Read on server side
        let received = server_socket.read_packet().await.unwrap();

        assert_eq!(received.header(), 42);
        assert_eq!(received.payload().as_ref(), b"Hello, Server!");

        write_task.await.unwrap();
    }
}
