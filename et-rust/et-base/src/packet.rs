/// Packet structure for ET protocol
///
/// Packets consist of:
/// - 1 byte: encrypted flag (0 or 1)
/// - 1 byte: header/type
/// - variable: payload

use crate::constants::PACKET_HEADER_SIZE;
use crate::crypto::CryptoHandler;
use crate::error::{EtError, Result};
use bytes::{BufMut, Bytes, BytesMut};

/// A network packet in the ET protocol
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    encrypted: bool,
    header: u8,
    payload: Bytes,
}

impl Packet {
    /// Create a new unencrypted packet
    pub fn new(header: u8, payload: impl Into<Bytes>) -> Self {
        Packet {
            encrypted: false,
            header,
            payload: payload.into(),
        }
    }

    /// Create a packet from raw header and payload
    pub fn with_encryption(encrypted: bool, header: u8, payload: impl Into<Bytes>) -> Self {
        Packet {
            encrypted,
            header,
            payload: payload.into(),
        }
    }

    /// Deserialize a packet from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < PACKET_HEADER_SIZE {
            return Err(EtError::InvalidPacket(format!(
                "Packet too small: {} bytes",
                data.len()
            )));
        }

        let encrypted = data[0] != 0;
        let header = data[1];
        let payload = Bytes::copy_from_slice(&data[2..]);

        Ok(Packet {
            encrypted,
            header,
            payload,
        })
    }

    /// Serialize packet to bytes
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_HEADER_SIZE + self.payload.len());
        buf.put_u8(if self.encrypted { 1 } else { 0 });
        buf.put_u8(self.header);
        buf.put(self.payload.clone());
        buf.freeze()
    }

    /// Encrypt the packet using the provided crypto handler
    pub fn encrypt(&mut self, crypto: &CryptoHandler) -> Result<()> {
        if self.encrypted {
            return Err(EtError::InvalidState(
                "Tried to encrypt a packet that was already encrypted".into(),
            ));
        }

        let encrypted_payload = crypto.encrypt(&self.payload)?;
        self.payload = Bytes::from(encrypted_payload);
        self.encrypted = true;

        Ok(())
    }

    /// Decrypt the packet using the provided crypto handler
    pub fn decrypt(&mut self, crypto: &CryptoHandler) -> Result<()> {
        if !self.encrypted {
            return Err(EtError::InvalidState(
                "Tried to decrypt a packet that wasn't encrypted".into(),
            ));
        }

        let decrypted_payload = crypto.decrypt(&self.payload)?;
        self.payload = Bytes::from(decrypted_payload);
        self.encrypted = false;

        Ok(())
    }

    /// Check if packet is encrypted
    pub fn is_encrypted(&self) -> bool {
        self.encrypted
    }

    /// Get packet header/type
    pub fn header(&self) -> u8 {
        self.header
    }

    /// Get packet payload
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }

    /// Get total packet length (header + payload)
    pub fn len(&self) -> usize {
        PACKET_HEADER_SIZE + self.payload.len()
    }

    /// Check if packet is empty (no payload)
    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }

    /// Consume packet and return payload
    pub fn into_payload(self) -> Bytes {
        self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::CRYPTO_KEY_BYTES;

    #[test]
    fn test_packet_creation() {
        let packet = Packet::new(42, b"hello".to_vec());
        assert_eq!(packet.header(), 42);
        assert_eq!(packet.payload().as_ref(), b"hello");
        assert!(!packet.is_encrypted());
    }

    #[test]
    fn test_packet_serialization() {
        let packet = Packet::new(10, b"test".to_vec());
        let bytes = packet.to_bytes();

        assert_eq!(bytes[0], 0); // not encrypted
        assert_eq!(bytes[1], 10); // header
        assert_eq!(&bytes[2..], b"test");

        let deserialized = Packet::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized, packet);
    }

    #[test]
    fn test_packet_encryption() {
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto = CryptoHandler::new(&key, 0).unwrap();

        let mut packet = Packet::new(5, b"secret message".to_vec());
        assert!(!packet.is_encrypted());

        packet.encrypt(&crypto).unwrap();
        assert!(packet.is_encrypted());

        // Encrypted payload should be longer (includes MAC)
        assert!(packet.payload().len() > b"secret message".len());
    }

    #[test]
    fn test_packet_roundtrip_with_encryption() {
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto1 = CryptoHandler::new(&key, 0).unwrap();
        let crypto2 = CryptoHandler::new(&key, 0).unwrap();

        let mut packet = Packet::new(7, b"test data".to_vec());
        let original_payload = packet.payload().clone();

        // Encrypt
        packet.encrypt(&crypto1).unwrap();
        assert!(packet.is_encrypted());

        // Serialize
        let bytes = packet.to_bytes();

        // Deserialize
        let mut received = Packet::from_bytes(&bytes).unwrap();
        assert!(received.is_encrypted());
        assert_eq!(received.header(), 7);

        // Decrypt
        received.decrypt(&crypto2).unwrap();
        assert!(!received.is_encrypted());
        assert_eq!(received.payload(), &original_payload);
    }

    #[test]
    fn test_double_encrypt_error() {
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto = CryptoHandler::new(&key, 0).unwrap();

        let mut packet = Packet::new(1, b"data".to_vec());
        packet.encrypt(&crypto).unwrap();

        // Second encryption should fail
        assert!(packet.encrypt(&crypto).is_err());
    }

    #[test]
    fn test_decrypt_unencrypted_error() {
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto = CryptoHandler::new(&key, 0).unwrap();

        let mut packet = Packet::new(1, b"data".to_vec());

        // Decrypting unencrypted packet should fail
        assert!(packet.decrypt(&crypto).is_err());
    }
}
