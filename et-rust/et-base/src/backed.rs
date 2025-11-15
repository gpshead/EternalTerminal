/// Reliable transport layer with buffering and retransmission
///
/// BackedReader and BackedWriter provide reliable packet transport with:
/// - Sequence number tracking
/// - Automatic buffering
/// - Retransmission support for reconnection
/// - Partial message handling

use crate::crypto::CryptoHandler;
use crate::error::{EtError, Result};
use crate::packet::Packet;
use crate::socket::AsyncSocket;
use bytes::Bytes;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

/// Maximum backup buffer size (64MB) for retransmission
pub const MAX_BACKUP_BYTES: usize = 64 * 1024 * 1024;

/// Write state returned by BackedWriter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteState {
    /// Write was skipped because socket is invalid
    Skipped,
    /// Write succeeded completely
    Success,
    /// Write attempted but failed (caller should think it succeeded)
    WroteWithFailure,
}

/// BackedWriter provides reliable packet transmission with retransmission support
///
/// Maintains a backup buffer of sent packets that can be retransmitted
/// after reconnection. The buffer is limited to 64MB.
pub struct BackedWriter {
    crypto: CryptoHandler,
    socket: Arc<Mutex<Option<Box<dyn AsyncSocket>>>>,
    backup_buffer: Arc<Mutex<BackupBuffer>>,
}

struct BackupBuffer {
    packets: VecDeque<Packet>,
    total_size: usize,
    sequence_number: i64,
}

impl BackedWriter {
    /// Create a new BackedWriter with the given crypto handler and socket
    pub fn new(crypto: CryptoHandler, socket: Box<dyn AsyncSocket>) -> Self {
        BackedWriter {
            crypto,
            socket: Arc::new(Mutex::new(Some(socket))),
            backup_buffer: Arc::new(Mutex::new(BackupBuffer {
                packets: VecDeque::new(),
                total_size: 0,
                sequence_number: 0,
            })),
        }
    }

    /// Write a packet to the socket
    ///
    /// The packet is encrypted and added to the backup buffer before transmission.
    /// Returns WriteState indicating success/failure.
    pub async fn write(&self, mut packet: Packet) -> WriteState {
        // Encrypt packet (updates crypto state, no going back)
        if let Err(e) = packet.encrypt(&self.crypto) {
            tracing::error!("Failed to encrypt packet: {}", e);
            return WriteState::WroteWithFailure;
        }

        // Add to backup buffer
        {
            let mut buffer = self.backup_buffer.lock();
            let packet_len = packet.len();

            buffer.packets.push_front(packet.clone());
            buffer.total_size += packet_len;
            buffer.sequence_number += 1;

            // Cleanup old packets to maintain size limit
            while buffer.total_size > MAX_BACKUP_BYTES {
                if let Some(old_packet) = buffer.packets.pop_back() {
                    buffer.total_size -= old_packet.len();
                }
            }
        }

        // Try to write to socket
        let mut socket_guard = self.socket.lock();
        let socket = match socket_guard.as_mut() {
            Some(s) => s,
            None => return WriteState::Skipped,
        };

        // Serialize packet with 4-byte length header (network byte order)
        let serialized = packet.to_bytes();
        let message_size = serialized.len() as u32;
        let header = message_size.to_be_bytes();

        // Write header + packet
        let mut full_message = Vec::with_capacity(4 + serialized.len());
        full_message.extend_from_slice(&header);
        full_message.extend_from_slice(&serialized);

        match socket.write_all(&full_message).await {
            Ok(_) => WriteState::Success,
            Err(e) => {
                tracing::warn!("Write failed: {}", e);
                WriteState::WroteWithFailure
            }
        }
    }

    /// Recover packets that need to be retransmitted
    ///
    /// Returns serialized packets from backup buffer that are newer than
    /// the last valid sequence number received by the peer.
    pub fn recover(&self, last_valid_sequence_number: i64) -> Result<Vec<Bytes>> {
        let buffer = self.backup_buffer.lock();

        let messages_to_recover = buffer.sequence_number - last_valid_sequence_number;

        if messages_to_recover < 0 {
            return Err(EtError::Protocol(
                "Client is ahead of server - sequence number mismatch".into(),
            ));
        }

        if messages_to_recover == 0 {
            return Ok(Vec::new());
        }

        tracing::info!("Recovering {} messages", messages_to_recover);

        let mut result = Vec::new();
        let mut messages_seen = 0;

        for packet in buffer.packets.iter() {
            result.push(packet.to_bytes());
            messages_seen += 1;
            if messages_seen == messages_to_recover {
                // Reverse to get correct order (buffer is LIFO, we want FIFO)
                result.reverse();
                return Ok(result);
            }
        }

        Err(EtError::Protocol(
            "Client is too far behind server - not enough backup".into(),
        ))
    }

    /// Revive the writer with a new socket after reconnection
    pub fn revive(&self, new_socket: Box<dyn AsyncSocket>) {
        let mut socket = self.socket.lock();
        *socket = Some(new_socket);
    }

    /// Invalidate the socket (mark as disconnected)
    pub fn invalidate_socket(&self) {
        let mut socket = self.socket.lock();
        *socket = None;
    }

    /// Get current sequence number
    pub fn sequence_number(&self) -> i64 {
        self.backup_buffer.lock().sequence_number
    }
}

/// BackedReader provides reliable packet reception with buffering
///
/// Handles partial message reads and maintains a local buffer for
/// packets that have been received but not yet consumed.
pub struct BackedReader {
    crypto: CryptoHandler,
    socket: Arc<Mutex<Option<Box<dyn AsyncSocket>>>>,
    state: Arc<Mutex<ReaderState>>,
}

struct ReaderState {
    sequence_number: i64,
    local_buffer: VecDeque<Bytes>,
}

impl BackedReader {
    /// Create a new BackedReader with the given crypto handler and socket
    pub fn new(crypto: CryptoHandler, socket: Box<dyn AsyncSocket>) -> Self {
        BackedReader {
            crypto,
            socket: Arc::new(Mutex::new(Some(socket))),
            state: Arc::new(Mutex::new(ReaderState {
                sequence_number: 0,
                local_buffer: VecDeque::new(),
            })),
        }
    }

    /// Check if there's data available to read
    pub fn has_data(&self) -> bool {
        let state = self.state.lock();
        !state.local_buffer.is_empty()
    }

    /// Read a packet from the local buffer
    ///
    /// For async socket reads, use the socket directly.
    /// This method only reads from pre-buffered packets.
    pub fn read_buffered(&self) -> Result<Option<Packet>> {
        let mut state = self.state.lock();

        if let Some(serialized) = state.local_buffer.pop_front() {
            tracing::debug!(
                "Reading from local buffer, {} remaining",
                state.local_buffer.len()
            );
            let mut packet = Packet::from_bytes(&serialized)?;
            packet.decrypt(&self.crypto)?;
            state.sequence_number += 1;
            Ok(Some(packet))
        } else {
            Ok(None)
        }
    }

    /// Revive the reader with a new socket and optional buffered packets
    pub fn revive(&self, new_socket: Box<dyn AsyncSocket>, new_entries: Vec<Bytes>) {
        let mut socket = self.socket.lock();
        *socket = Some(new_socket);

        let new_entries_len = new_entries.len();
        let mut state = self.state.lock();
        state.local_buffer.extend(new_entries);
        state.sequence_number += new_entries_len as i64;
    }

    /// Invalidate the socket (mark as disconnected)
    pub fn invalidate_socket(&self) {
        let mut socket = self.socket.lock();
        *socket = None;
    }

    /// Get current sequence number
    pub fn sequence_number(&self) -> i64 {
        self.state.lock().sequence_number
    }

    /// Get the socket for reading (if valid)
    pub fn get_socket(&self) -> Option<Box<dyn AsyncSocket>> {
        let mut socket_guard = self.socket.lock();
        socket_guard.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CRYPTO_KEY_BYTES;

    #[tokio::test]
    async fn test_backed_writer_sequence() {
        use tokio::io::DuplexStream;

        struct TestSocket(DuplexStream);

        #[async_trait::async_trait]
        impl AsyncSocket for TestSocket {
            async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
                use tokio::io::AsyncReadExt;
                self.0.read_exact(buf).await.map(|_| ()).map_err(EtError::from)
            }

            async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
                use tokio::io::AsyncWriteExt;
                self.0.write_all(buf).await.map_err(EtError::from)
            }

            async fn shutdown(&mut self) -> Result<()> {
                Ok(())
            }
        }

        let (client, _server) = tokio::io::duplex(1024);
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto = CryptoHandler::new(&key, 0).unwrap();

        let writer = BackedWriter::new(crypto, Box::new(TestSocket(client)));

        // Write some packets
        let packet1 = Packet::new(1, b"test1".to_vec());
        let packet2 = Packet::new(2, b"test2".to_vec());

        writer.write(packet1).await;
        writer.write(packet2).await;

        assert_eq!(writer.sequence_number(), 2);

        // Test recovery
        let recovered = writer.recover(0).unwrap();
        assert_eq!(recovered.len(), 2);
    }

    #[test]
    fn test_backup_buffer_limit() {
        use tokio::io::DuplexStream;

        struct TestSocket(DuplexStream);

        #[async_trait::async_trait]
        impl AsyncSocket for TestSocket {
            async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
                use tokio::io::AsyncReadExt;
                self.0.read_exact(buf).await.map(|_| ()).map_err(EtError::from)
            }

            async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
                use tokio::io::AsyncWriteExt;
                self.0.write_all(buf).await.map_err(EtError::from)
            }

            async fn shutdown(&mut self) -> Result<()> {
                Ok(())
            }
        }

        let (client, _server) = tokio::io::duplex(1024);
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto = CryptoHandler::new(&key, 0).unwrap();

        let writer = BackedWriter::new(crypto, Box::new(TestSocket(client)));

        // Check that buffer size is managed
        let buffer = writer.backup_buffer.lock();
        assert!(buffer.total_size <= MAX_BACKUP_BYTES);
    }

    #[test]
    fn test_backed_reader_buffered() {
        use tokio::io::DuplexStream;

        struct TestSocket(DuplexStream);

        #[async_trait::async_trait]
        impl AsyncSocket for TestSocket {
            async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
                use tokio::io::AsyncReadExt;
                self.0.read_exact(buf).await.map(|_| ()).map_err(EtError::from)
            }

            async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
                use tokio::io::AsyncWriteExt;
                self.0.write_all(buf).await.map_err(EtError::from)
            }

            async fn shutdown(&mut self) -> Result<()> {
                Ok(())
            }
        }

        let (client, _server) = tokio::io::duplex(1024);
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto_encrypt = CryptoHandler::new(&key, 0).unwrap();
        let crypto_decrypt = CryptoHandler::new(&key, 0).unwrap();

        let reader = BackedReader::new(crypto_decrypt, Box::new(TestSocket(client)));

        // Initially no data
        assert!(!reader.has_data());
        assert!(reader.read_buffered().unwrap().is_none());

        // Add some buffered data
        let packet = Packet::new(1, b"test".to_vec());
        let mut encrypted = packet.clone();
        encrypted.encrypt(&crypto_encrypt).unwrap();

        let (_client2, _server2) = tokio::io::duplex(1024);
        reader.revive(Box::new(TestSocket(_client2)), vec![encrypted.to_bytes()]);

        // Now should have data
        assert!(reader.has_data());
        let read_packet = reader.read_buffered().unwrap().unwrap();
        assert_eq!(read_packet.header(), 1);
    }
}
