/// Cryptography handler using libsodium (sodiumoxide)
///
/// Provides authenticated encryption using crypto_secretbox (XSalsa20 + Poly1305)

use crate::error::{EtError, Result};
use parking_lot::Mutex;
use sodiumoxide::crypto::secretbox;
use std::sync::Arc;

pub const CRYPTO_KEY_BYTES: usize = secretbox::KEYBYTES;
pub const CRYPTO_NONCE_BYTES: usize = secretbox::NONCEBYTES;
pub const CRYPTO_MAC_BYTES: usize = secretbox::MACBYTES;

/// Handler for encrypting and decrypting packets
///
/// Each handler maintains a nonce counter that is incremented for each
/// encryption/decryption operation. The nonce MSB distinguishes between
/// client->server (0) and server->client (1) traffic.
#[derive(Clone)]
pub struct CryptoHandler {
    inner: Arc<CryptoHandlerInner>,
}

struct CryptoHandlerInner {
    key: secretbox::Key,
    nonce: Mutex<[u8; CRYPTO_NONCE_BYTES]>,
}

impl CryptoHandler {
    /// Create a new CryptoHandler with the given key and nonce MSB
    ///
    /// # Arguments
    /// * `key` - 32-byte secret key
    /// * `nonce_msb` - Most significant byte of nonce (0 for client, 1 for server)
    pub fn new(key: &[u8], nonce_msb: u8) -> Result<Self> {
        // Initialize sodium
        sodiumoxide::init().map_err(|_| EtError::Crypto("Failed to initialize libsodium".into()))?;

        if key.len() != CRYPTO_KEY_BYTES {
            return Err(EtError::InvalidKeyLength {
                expected: CRYPTO_KEY_BYTES,
                got: key.len(),
            });
        }

        let mut key_bytes = [0u8; CRYPTO_KEY_BYTES];
        key_bytes.copy_from_slice(key);
        let key = secretbox::Key(key_bytes);

        let mut nonce = [0u8; CRYPTO_NONCE_BYTES];
        nonce[CRYPTO_NONCE_BYTES - 1] = nonce_msb;

        Ok(CryptoHandler {
            inner: Arc::new(CryptoHandlerInner {
                key,
                nonce: Mutex::new(nonce),
            }),
        })
    }

    /// Encrypt a buffer using crypto_secretbox
    ///
    /// Returns the encrypted data (includes MAC)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce_guard = self.inner.nonce.lock();
        self.increment_nonce(&mut nonce_guard);

        let nonce = secretbox::Nonce(*nonce_guard);
        let ciphertext = secretbox::seal(plaintext, &nonce, &self.inner.key);

        Ok(ciphertext)
    }

    /// Decrypt a buffer using crypto_secretbox
    ///
    /// Returns the decrypted data or an error if authentication fails
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce_guard = self.inner.nonce.lock();
        self.increment_nonce(&mut nonce_guard);

        let nonce = secretbox::Nonce(*nonce_guard);

        secretbox::open(ciphertext, &nonce, &self.inner.key)
            .map_err(|_| EtError::DecryptionFailed("Failed to decrypt packet. Possible key mismatch?".into()))
    }

    /// Increment the nonce counter (little-endian increment)
    fn increment_nonce(&self, nonce: &mut [u8; CRYPTO_NONCE_BYTES]) {
        for byte in nonce.iter_mut() {
            *byte = byte.wrapping_add(1);
            if *byte != 0 {
                // No overflow, we're done
                break;
            }
            // Overflow to next byte
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_roundtrip() {
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto = CryptoHandler::new(&key, 0).unwrap();

        let plaintext = b"Hello, World!";
        let ciphertext = crypto.encrypt(plaintext).unwrap();

        // Ciphertext should be longer due to MAC
        assert_eq!(ciphertext.len(), plaintext.len() + CRYPTO_MAC_BYTES);

        // Create a new crypto handler with same key for decryption
        let crypto2 = CryptoHandler::new(&key, 0).unwrap();
        let decrypted = crypto2.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_nonce_increment() {
        let key = vec![0u8; CRYPTO_KEY_BYTES];
        let crypto = CryptoHandler::new(&key, 0).unwrap();

        let plaintext = b"test";

        // Encrypt twice
        let ct1 = crypto.encrypt(plaintext).unwrap();
        let ct2 = crypto.encrypt(plaintext).unwrap();

        // Ciphertexts should be different due to different nonces
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn test_invalid_key_length() {
        let key = vec![0u8; 16]; // Wrong length
        let result = CryptoHandler::new(&key, 0);
        assert!(result.is_err());
    }
}
