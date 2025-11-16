/// Eternal Terminal - Terminal Handling Library
///
/// This library provides terminal handling functionality for the Eternal Terminal
/// Rust implementation, including PTY (pseudo-terminal) management, terminal
/// raw mode handling, and SSH client capabilities.

pub mod pty;
pub mod raw_mode;
pub mod ssh;
pub mod config;

// Re-export commonly used types
pub use pty::{PtyMaster, TerminalSize};
pub use raw_mode::{RawModeGuard, is_terminal};
pub use ssh::{SshClient, SshConfig, parse_ssh_target};
pub use config::{Config, HostSettings, PortForward};
