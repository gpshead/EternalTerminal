/// Core constants for the Eternal Terminal protocol

/// The ET protocol version supported by this implementation
pub const PROTOCOL_VERSION: i32 = 6;

/// Nonces for CryptoHandler - used to distinguish client->server vs server->client traffic
pub const CLIENT_SERVER_NONCE_MSB: u8 = 0;
pub const SERVER_CLIENT_NONCE_MSB: u8 = 1;

/// System SSH config file paths
pub const SYSTEM_SSH_CONFIG_PATH: &str = "/etc/ssh/ssh_config";
pub const USER_SSH_CONFIG_PATH: &str = "/.ssh/config";

/// Keepalive configuration
/// Client sends keepalive every 5 seconds
pub const MAX_CLIENT_KEEP_ALIVE_DURATION: u64 = 5;

/// Server times out connection if no data received for 11 seconds
/// This is at least double MAX_CLIENT_KEEP_ALIVE_DURATION to allow enough time
pub const SERVER_KEEP_ALIVE_DURATION: u64 = 11;

/// Default server port
pub const DEFAULT_SERVER_PORT: u16 = 2022;

/// Packet header size (encrypted flag + header byte)
pub const PACKET_HEADER_SIZE: usize = 2;

/// Backed buffer configuration
/// Maximum amount of sent data to keep for retransmission (64MB)
pub const MAX_BACKED_BUFFER_SIZE: usize = 64 * 1024 * 1024;

/// Size of sequence number space
pub const SEQUENCE_NUMBER_MODULO: i32 = i32::MAX;
