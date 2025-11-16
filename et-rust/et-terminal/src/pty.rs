/// PTY (Pseudo-Terminal) Management
///
/// This module provides functionality for creating and managing pseudo-terminals (PTYs).
/// PTYs are essential for terminal emulation and allow programs to interact with a shell
/// or other terminal-based programs as if they were running in a real terminal.

use anyhow::{Context, Result};
use nix::errno::Errno;
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use nix::ioctl_read_bad;
use nix::ioctl_write_ptr_bad;
use nix::libc::{self, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use nix::pty::{grantpt, posix_openpt, ptsname, unlockpt, PtyMaster as NixPtyMaster};
use nix::sys::signal::{signal, SigHandler, Signal};
use nix::unistd::{close, dup2, execvp, fork, setsid, ForkResult, Pid};
use std::ffi::CString;
use std::os::unix::io::{AsRawFd, BorrowedFd, RawFd};
use tracing::{debug, error, info};

// Terminal window size ioctl
ioctl_read_bad!(tiocgwinsz, libc::TIOCGWINSZ, libc::winsize);
ioctl_write_ptr_bad!(tiocswinsz, libc::TIOCSWINSZ, libc::winsize);

/// Terminal size in rows and columns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

impl TerminalSize {
    /// Get the terminal size from a file descriptor
    pub fn from_fd(fd: RawFd) -> Result<Self> {
        let mut winsize = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        unsafe {
            let borrowed_fd = BorrowedFd::borrow_raw(fd);
            tiocgwinsz(borrowed_fd.as_raw_fd(), &mut winsize)
                .context("Failed to get terminal window size")?;
        }

        Ok(Self {
            rows: winsize.ws_row,
            cols: winsize.ws_col,
            pixel_width: winsize.ws_xpixel,
            pixel_height: winsize.ws_ypixel,
        })
    }

    /// Get the terminal size from stdin
    pub fn from_stdin() -> Result<Self> {
        Self::from_fd(STDIN_FILENO)
    }

    /// Create a winsize structure for ioctl
    fn to_winsize(&self) -> libc::winsize {
        libc::winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: self.pixel_width,
            ws_ypixel: self.pixel_height,
        }
    }
}

/// PTY Master - represents the master side of a pseudo-terminal
pub struct PtyMaster {
    master: NixPtyMaster,
    size: TerminalSize,
    child_pid: Option<Pid>,
}

impl PtyMaster {
    /// Open a new PTY master
    pub fn open() -> Result<Self> {
        let master = posix_openpt(OFlag::O_RDWR | OFlag::O_NOCTTY)
            .context("Failed to open PTY master")?;

        grantpt(&master).context("Failed to grant PTY")?;
        unlockpt(&master).context("Failed to unlock PTY")?;

        debug!("Opened PTY master: fd={}", master.as_raw_fd());

        Ok(Self {
            master,
            size: TerminalSize::default(),
            child_pid: None,
        })
    }

    /// Get the file descriptor of the PTY master
    pub fn as_raw_fd(&self) -> RawFd {
        self.master.as_raw_fd()
    }

    /// Set the terminal size
    pub fn set_size(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.size.rows = rows;
        self.size.cols = cols;

        let winsize = self.size.to_winsize();
        let fd = self.master.as_raw_fd();

        unsafe {
            tiocswinsz(fd, &winsize)
                .context("Failed to set terminal window size")?;
        }

        debug!(
            "Set PTY size: {}x{} ({}x{} pixels)",
            rows, cols, self.size.pixel_width, self.size.pixel_height
        );

        Ok(())
    }

    /// Set the terminal size from another terminal
    pub fn set_size_from_fd(&mut self, fd: RawFd) -> Result<()> {
        self.size = TerminalSize::from_fd(fd)?;

        let winsize = self.size.to_winsize();
        let master_fd = self.master.as_raw_fd();

        unsafe {
            tiocswinsz(master_fd, &winsize)
                .context("Failed to set terminal window size")?;
        }

        Ok(())
    }

    /// Get the current terminal size
    pub fn get_size(&self) -> TerminalSize {
        self.size
    }

    /// Get the PTY slave path
    pub fn slave_name(&self) -> Result<String> {
        unsafe { ptsname(&self.master) }.context("Failed to get PTY slave name")
    }

    /// Read from the PTY master (non-blocking)
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // Set non-blocking mode
        let fd = self.master.as_raw_fd();
        let flags = fcntl(fd, FcntlArg::F_GETFL).context("Failed to get PTY flags")?;
        let mut oflags = OFlag::from_bits_truncate(flags);
        oflags.insert(OFlag::O_NONBLOCK);
        fcntl(fd, FcntlArg::F_SETFL(oflags)).context("Failed to set PTY to non-blocking")?;

        match nix::unistd::read(fd, buf) {
            Ok(n) => Ok(n),
            Err(Errno::EAGAIN) => Ok(0),
            Err(e) => Err(e).context("Failed to read from PTY"),
        }
    }

    /// Write to the PTY master
    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        nix::unistd::write(self.master.as_raw_fd(), buf).context("Failed to write to PTY")
    }

    /// Spawn a shell in the PTY slave
    ///
    /// # Arguments
    /// * `shell` - Path to the shell executable (e.g., "/bin/bash")
    /// * `command` - Optional command to execute (if None, starts interactive shell)
    /// * `env` - Optional environment variables to set
    ///
    /// # Returns
    /// The PID of the child process
    pub fn spawn_shell(
        &mut self,
        shell: &str,
        command: Option<&str>,
        env: Option<&[(String, String)]>,
    ) -> Result<Pid> {
        let slave_name = self.slave_name()?;

        debug!("Spawning shell: {} in PTY: {}", shell, slave_name);

        match unsafe { fork() }.context("Failed to fork process")? {
            ForkResult::Parent { child } => {
                // Parent process
                self.child_pid = Some(child);
                info!("Spawned shell process: pid={}", child);
                Ok(child)
            }
            ForkResult::Child => {
                // Child process
                self.setup_child_process(&slave_name, shell, command, env)
            }
        }
    }

    /// Set up the child process in the PTY slave
    fn setup_child_process(
        &self,
        slave_name: &str,
        shell: &str,
        command: Option<&str>,
        env: Option<&[(String, String)]>,
    ) -> Result<Pid> {
        // Create a new session and become the session leader
        setsid().context("Failed to create new session")?;

        // Reset signal handlers to default
        unsafe {
            signal(Signal::SIGCHLD, SigHandler::SigDfl).ok();
            signal(Signal::SIGHUP, SigHandler::SigDfl).ok();
            signal(Signal::SIGINT, SigHandler::SigDfl).ok();
            signal(Signal::SIGQUIT, SigHandler::SigDfl).ok();
            signal(Signal::SIGTERM, SigHandler::SigDfl).ok();
            signal(Signal::SIGALRM, SigHandler::SigDfl).ok();
        }

        // Open the PTY slave
        let slave_fd = nix::fcntl::open(
            std::path::Path::new(slave_name),
            OFlag::O_RDWR,
            nix::sys::stat::Mode::empty(),
        )
        .context("Failed to open PTY slave")?;

        // Make the PTY slave the controlling terminal
        unsafe {
            if libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) < 0 {
                error!("Failed to set controlling terminal");
            }
        }

        // Redirect stdin, stdout, stderr to the PTY slave
        dup2(slave_fd, STDIN_FILENO).context("Failed to dup stdin")?;
        dup2(slave_fd, STDOUT_FILENO).context("Failed to dup stdout")?;
        dup2(slave_fd, STDERR_FILENO).context("Failed to dup stderr")?;

        // Close the original slave fd if it's not one of the standard fds
        if slave_fd > STDERR_FILENO {
            close(slave_fd).ok();
        }

        // Set environment variables
        if let Some(env_vars) = env {
            for (key, value) in env_vars {
                std::env::set_var(key, value);
            }
        }

        // Ensure TERM is set
        if std::env::var("TERM").is_err() {
            std::env::set_var("TERM", "xterm-256color");
        }

        // Prepare arguments for execvp
        let shell_cstring = CString::new(shell).context("Invalid shell path")?;

        let args: Vec<CString> = if let Some(cmd) = command {
            // Execute command: shell -c "command"
            vec![
                shell_cstring.clone(),
                CString::new("-c").unwrap(),
                CString::new(cmd).context("Invalid command")?,
            ]
        } else {
            // Interactive shell: shell -l (login shell)
            vec![shell_cstring.clone(), CString::new("-l").unwrap()]
        };

        // Execute the shell (this replaces the current process)
        execvp(&shell_cstring, &args).context("Failed to execute shell")?;

        // This should never be reached
        unreachable!()
    }

    /// Get the child process PID (if spawned)
    pub fn child_pid(&self) -> Option<Pid> {
        self.child_pid
    }

    /// Check if the child process is still running
    pub fn is_child_alive(&self) -> bool {
        if let Some(pid) = self.child_pid {
            // Send signal 0 to check if process exists
            match nix::sys::signal::kill(pid, None) {
                Ok(_) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Wait for the child process to exit
    pub fn wait_child(&self) -> Result<nix::sys::wait::WaitStatus> {
        if let Some(pid) = self.child_pid {
            nix::sys::wait::waitpid(pid, None).context("Failed to wait for child process")
        } else {
            anyhow::bail!("No child process to wait for")
        }
    }
}

impl Drop for PtyMaster {
    fn drop(&mut self) {
        debug!("Closing PTY master: fd={}", self.master.as_raw_fd());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_size_default() {
        let size = TerminalSize::default();
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }

    #[test]
    fn test_pty_open() {
        let pty = PtyMaster::open().expect("Failed to open PTY");
        assert!(pty.as_raw_fd() >= 0);
    }

    #[test]
    fn test_pty_slave_name() {
        let pty = PtyMaster::open().expect("Failed to open PTY");
        let name = pty.slave_name().expect("Failed to get slave name");
        assert!(name.starts_with("/dev/pts/") || name.starts_with("/dev/pty"));
    }

    #[test]
    fn test_pty_set_size() {
        let mut pty = PtyMaster::open().expect("Failed to open PTY");
        pty.set_size(50, 120).expect("Failed to set size");
        assert_eq!(pty.get_size().rows, 50);
        assert_eq!(pty.get_size().cols, 120);
    }

    #[test]
    fn test_pty_write_read() {
        let pty = PtyMaster::open().expect("Failed to open PTY");

        // Note: This test may not work as expected because there's no process
        // reading from the slave end. In a real scenario, we'd spawn a shell.
        let data = b"test data";
        let written = pty.write(data).expect("Failed to write");
        assert_eq!(written, data.len());
    }
}
