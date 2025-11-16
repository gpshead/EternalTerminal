/// Terminal Raw Mode Management
///
/// This module provides functionality for switching a terminal into raw mode,
/// which is necessary for terminal emulation. In raw mode, input is passed
/// directly to the application without line buffering or special character processing.

use anyhow::{Context, Result};
use nix::sys::termios::{tcgetattr, tcsetattr, SetArg, Termios};
use nix::libc::STDIN_FILENO;
use std::os::unix::io::{BorrowedFd, RawFd};
use tracing::debug;

/// RAII guard that enables raw mode and restores the original terminal settings on drop
pub struct RawModeGuard {
    fd: RawFd,
    original_termios: Termios,
}

impl RawModeGuard {
    /// Enable raw mode on the specified file descriptor
    ///
    /// # Arguments
    /// * `fd` - File descriptor of the terminal (typically STDIN_FILENO)
    ///
    /// # Returns
    /// A guard that will restore the original terminal settings when dropped
    pub fn enable(fd: RawFd) -> Result<Self> {
        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(fd) };
        let original_termios = tcgetattr(&borrowed_fd).context("Failed to get terminal attributes")?;

        let mut raw_termios = original_termios.clone();

        // Enter raw mode
        nix::sys::termios::cfmakeraw(&mut raw_termios);

        tcsetattr(&borrowed_fd, SetArg::TCSAFLUSH, &raw_termios)
            .context("Failed to set terminal to raw mode")?;

        debug!("Enabled raw mode on fd {}", fd);

        Ok(Self {
            fd,
            original_termios,
        })
    }

    /// Enable raw mode on stdin
    pub fn enable_stdin() -> Result<Self> {
        Self::enable(STDIN_FILENO)
    }

    /// Manually restore the original terminal settings
    pub fn restore(&self) -> Result<()> {
        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(self.fd) };
        tcsetattr(&borrowed_fd, SetArg::TCSAFLUSH, &self.original_termios)
            .context("Failed to restore terminal attributes")
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if let Err(e) = self.restore() {
            eprintln!("Failed to restore terminal settings: {}", e);
        } else {
            debug!("Restored terminal settings on fd {}", self.fd);
        }
    }
}

/// Helper function to check if a file descriptor refers to a terminal
pub fn is_terminal(fd: RawFd) -> bool {
    nix::unistd::isatty(fd).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_terminal() {
        // Note: This test may fail in non-interactive environments
        // where stdin is not a terminal
        let result = is_terminal(STDIN_FILENO);
        // We don't assert true/false because it depends on the test environment
        println!("STDIN is terminal: {}", result);
    }

    #[test]
    fn test_raw_mode_enable_disable() {
        // This test only works if stdin is a terminal
        if is_terminal(STDIN_FILENO) {
            let borrowed_fd = unsafe { BorrowedFd::borrow_raw(STDIN_FILENO) };
            let original = tcgetattr(&borrowed_fd).expect("Failed to get termios");

            {
                let _guard = RawModeGuard::enable_stdin().expect("Failed to enable raw mode");
                let borrowed_fd = unsafe { BorrowedFd::borrow_raw(STDIN_FILENO) };
                let raw = tcgetattr(&borrowed_fd).expect("Failed to get termios");

                // Verify that some raw mode flags are set
                assert!(raw.local_flags != original.local_flags);
            }

            // After guard is dropped, settings should be restored
            let borrowed_fd = unsafe { BorrowedFd::borrow_raw(STDIN_FILENO) };
            let restored = tcgetattr(&borrowed_fd).expect("Failed to get termios");
            assert_eq!(restored.local_flags, original.local_flags);
        }
    }
}
