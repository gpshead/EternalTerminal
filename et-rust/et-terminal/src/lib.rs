/// Eternal Terminal - Terminal Handling Library
///
/// This library provides terminal handling functionality for the Eternal Terminal
/// Rust implementation, including PTY (pseudo-terminal) management and terminal
/// raw mode handling.

pub mod pty;
pub mod raw_mode;

// Re-export commonly used types
pub use pty::{PtyMaster, TerminalSize};
pub use raw_mode::{RawModeGuard, is_terminal};
