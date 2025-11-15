//! Base library for Eternal Terminal
//!
//! This crate provides the core abstractions and functionality for the ET protocol:
//! - Constants and configuration
//! - Error types
//! - Packet serialization and encryption
//! - Cryptography using libsodium
//! - Utility functions

pub mod constants;
pub mod crypto;
pub mod error;
pub mod packet;
pub mod utils;

// Re-export commonly used types
pub use constants::*;
pub use crypto::{CryptoHandler, CRYPTO_KEY_BYTES, CRYPTO_MAC_BYTES, CRYPTO_NONCE_BYTES};
pub use error::{EtError, Result};
pub use packet::Packet;

// Re-export protocol buffer types
pub use et_proto;
