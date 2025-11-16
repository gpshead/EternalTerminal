/// SSH Client Module
///
/// Provides SSH connection and remote command execution capabilities for ET.
/// This module handles:
/// - SSH connection establishment
/// - Authentication (public key, password, agent)
/// - Remote command execution
/// - Session management

use anyhow::{Context, Result};
use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// SSH connection configuration
#[derive(Debug, Clone)]
pub struct SshConfig {
    /// Remote host
    pub host: String,
    /// SSH port
    pub port: u16,
    /// Username
    pub user: String,
    /// Private key path (optional, will try default locations)
    pub identity_file: Option<PathBuf>,
    /// Password (if not using key auth)
    pub password: Option<String>,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 22,
            user: std::env::var("USER").unwrap_or_else(|_| "root".to_string()),
            identity_file: None,
            password: None,
        }
    }
}

/// SSH client for remote command execution
pub struct SshClient {
    session: Session,
    config: SshConfig,
}

impl SshClient {
    /// Connect to remote host via SSH
    pub fn connect(config: SshConfig) -> Result<Self> {
        info!("Connecting to SSH: {}@{}:{}", config.user, config.host, config.port);

        // Establish TCP connection
        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))
            .context("Failed to connect to SSH server")?;

        // Create SSH session
        let mut session = Session::new().context("Failed to create SSH session")?;
        session.set_tcp_stream(tcp);
        session.handshake().context("SSH handshake failed")?;

        debug!("SSH handshake completed");

        // Authenticate
        Self::authenticate(&mut session, &config)?;

        info!("SSH authentication successful");

        Ok(Self { session, config })
    }

    /// Authenticate SSH session
    fn authenticate(session: &mut Session, config: &SshConfig) -> Result<()> {
        // Try public key authentication first
        if let Some(ref key_path) = config.identity_file {
            debug!("Trying public key authentication: {:?}", key_path);
            if session.userauth_pubkey_file(&config.user, None, key_path, None).is_ok() {
                return Ok(());
            }
        }

        // Try default SSH key locations
        let default_keys = Self::get_default_key_paths();
        for key_path in &default_keys {
            if key_path.exists() {
                debug!("Trying default key: {:?}", key_path);
                if session.userauth_pubkey_file(&config.user, None, key_path, None).is_ok() {
                    return Ok(());
                }
            }
        }

        // Try SSH agent
        debug!("Trying SSH agent authentication");
        if session.userauth_agent(&config.user).is_ok() {
            return Ok(());
        }

        // Try password authentication if provided
        if let Some(ref password) = config.password {
            debug!("Trying password authentication");
            session.userauth_password(&config.user, password)
                .context("Password authentication failed")?;
            return Ok(());
        }

        anyhow::bail!("All authentication methods failed")
    }

    /// Get default SSH key paths
    fn get_default_key_paths() -> Vec<PathBuf> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let ssh_dir = Path::new(&home).join(".ssh");

        vec![
            ssh_dir.join("id_ed25519"),
            ssh_dir.join("id_rsa"),
            ssh_dir.join("id_ecdsa"),
            ssh_dir.join("id_dsa"),
        ]
    }

    /// Execute a command on the remote host
    pub fn execute_command(&self, command: &str) -> Result<String> {
        debug!("Executing remote command: {}", command);

        let mut channel = self.session.channel_session()
            .context("Failed to open SSH channel")?;

        channel.exec(command)
            .context("Failed to execute command")?;

        let mut output = String::new();
        channel.read_to_string(&mut output)
            .context("Failed to read command output")?;

        channel.wait_close()
            .context("Failed to close channel")?;

        let exit_status = channel.exit_status()
            .context("Failed to get exit status")?;

        debug!("Command exit status: {}", exit_status);

        if exit_status != 0 {
            warn!("Command failed with exit code: {}", exit_status);
        }

        Ok(output)
    }

    /// Execute a command and return the session for interactive use
    ///
    /// This keeps the channel open for bidirectional communication.
    /// The caller is responsible for managing the channel lifecycle.
    pub fn execute_interactive(&self, command: &str) -> Result<ssh2::Channel> {
        debug!("Executing interactive command: {}", command);

        let mut channel = self.session.channel_session()
            .context("Failed to open SSH channel")?;

        // Request a PTY for interactive sessions
        channel.request_pty("xterm-256color", None, None)
            .context("Failed to request PTY")?;

        channel.exec(command)
            .context("Failed to execute command")?;

        Ok(channel)
    }

    /// Check if the connection is still alive
    pub fn is_alive(&self) -> bool {
        self.session.authenticated()
    }

    /// Get the remote host
    pub fn host(&self) -> &str {
        &self.config.host
    }

    /// Get the username
    pub fn user(&self) -> &str {
        &self.config.user
    }
}

/// Parse SSH target string (user@host:port or user@host or host)
pub fn parse_ssh_target(target: &str) -> Result<(String, String, u16)> {
    let default_user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());

    // Split by '@' to separate user and host
    let (user, host_port) = if target.contains('@') {
        let parts: Vec<&str> = target.splitn(2, '@').collect();
        (parts[0].to_string(), parts[1])
    } else {
        (default_user, target)
    };

    // Split by ':' to separate host and port
    let (host, port) = if host_port.contains(':') {
        let parts: Vec<&str> = host_port.splitn(2, ':').collect();
        let port = parts[1].parse::<u16>()
            .context("Invalid port number")?;
        (parts[0].to_string(), port)
    } else {
        (host_port.to_string(), 22)
    };

    Ok((user, host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_target_full() {
        let (user, host, port) = parse_ssh_target("alice@example.com:2222").unwrap();
        assert_eq!(user, "alice");
        assert_eq!(host, "example.com");
        assert_eq!(port, 2222);
    }

    #[test]
    fn test_parse_ssh_target_no_port() {
        let (user, host, port) = parse_ssh_target("bob@example.com").unwrap();
        assert_eq!(user, "bob");
        assert_eq!(host, "example.com");
        assert_eq!(port, 22);
    }

    #[test]
    fn test_parse_ssh_target_no_user() {
        let (user, host, port) = parse_ssh_target("example.com").unwrap();
        // User will be current user from environment
        assert_eq!(host, "example.com");
        assert_eq!(port, 22);
    }

    #[test]
    fn test_parse_ssh_target_host_only() {
        let (user, host, port) = parse_ssh_target("localhost").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 22);
    }

    #[test]
    fn test_default_key_paths() {
        let paths = SshClient::get_default_key_paths();
        assert!(!paths.is_empty());
        // Should include common key types
        assert!(paths.iter().any(|p| p.to_str().unwrap().contains("id_ed25519")));
        assert!(paths.iter().any(|p| p.to_str().unwrap().contains("id_rsa")));
    }
}
