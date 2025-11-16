/// Configuration file support for ET
///
/// Supports reading configuration from:
/// - ~/.et/config.toml (user config)
/// - /etc/et/config.toml (system config)
///
/// Configuration format:
/// ```toml
/// # Default settings
/// [defaults]
/// ssh_port = 22
/// et_port = 2022
/// identity_file = "~/.ssh/id_rsa"
///
/// # Host-specific settings
/// [[hosts]]
/// pattern = "*.example.com"
/// ssh_port = 2222
/// identity_file = "~/.ssh/work_key"
/// jumphost = "bastion.example.com"
///
/// # Port forwarding rules
/// [[port_forwards]]
/// local_port = 8080
/// remote_host = "localhost"
/// remote_port = 80
/// ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub defaults: Defaults,

    #[serde(default)]
    pub hosts: Vec<HostConfig>,

    #[serde(default)]
    pub port_forwards: Vec<PortForward>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,

    #[serde(default = "default_et_port")]
    pub et_port: u16,

    pub identity_file: Option<String>,

    pub jumphost: Option<String>,

    #[serde(default)]
    pub verbose: bool,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            ssh_port: default_ssh_port(),
            et_port: default_et_port(),
            identity_file: None,
            jumphost: None,
            verbose: false,
        }
    }
}

fn default_ssh_port() -> u16 {
    22
}

fn default_et_port() -> u16 {
    2022
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    /// Pattern to match hostname (supports wildcards)
    pub pattern: String,

    pub ssh_port: Option<u16>,

    pub et_port: Option<u16>,

    pub identity_file: Option<String>,

    pub jumphost: Option<String>,

    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForward {
    /// Local port to listen on
    pub local_port: u16,

    /// Remote host to forward to
    pub remote_host: String,

    /// Remote port to forward to
    pub remote_port: u16,

    /// Optional: only for specific hosts
    pub for_host: Option<String>,
}

impl Config {
    /// Load configuration from default locations
    pub fn load() -> Result<Self> {
        // Try user config first
        if let Some(user_config_path) = Self::user_config_path() {
            if user_config_path.exists() {
                return Self::load_from_file(&user_config_path);
            }
        }

        // Try system config
        let system_config_path = PathBuf::from("/etc/et/config.toml");
        if system_config_path.exists() {
            return Self::load_from_file(&system_config_path);
        }

        // Return default config if no file found
        Ok(Self::default())
    }

    /// Load configuration from a specific file
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;

        Ok(config)
    }

    /// Get user config path (~/.et/config.toml)
    pub fn user_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".et").join("config.toml"))
    }

    /// Get configuration for a specific host
    pub fn get_host_config(&self, hostname: &str) -> HostSettings {
        let mut settings = HostSettings::from_defaults(&self.defaults);

        // Find matching host configs (in order, later ones override)
        for host_config in &self.hosts {
            if Self::matches_pattern(&host_config.pattern, hostname) {
                settings.apply_host_config(host_config);
            }
        }

        settings
    }

    /// Get port forwards applicable to a host
    pub fn get_port_forwards(&self, hostname: Option<&str>) -> Vec<PortForward> {
        self.port_forwards
            .iter()
            .filter(|pf| {
                if let Some(for_host) = &pf.for_host {
                    if let Some(host) = hostname {
                        Self::matches_pattern(for_host, host)
                    } else {
                        false
                    }
                } else {
                    true // No for_host means apply to all
                }
            })
            .cloned()
            .collect()
    }

    /// Check if hostname matches pattern (supports * wildcards)
    fn matches_pattern(pattern: &str, hostname: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') {
            // Simple wildcard matching
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return hostname.starts_with(prefix) && hostname.ends_with(suffix);
            }
        }

        pattern == hostname
    }
}

/// Resolved settings for a specific host
#[derive(Debug, Clone)]
pub struct HostSettings {
    pub ssh_port: u16,
    pub et_port: u16,
    pub identity_file: Option<PathBuf>,
    pub jumphost: Option<String>,
    pub user: Option<String>,
}

impl HostSettings {
    pub fn from_defaults(defaults: &Defaults) -> Self {
        Self {
            ssh_port: defaults.ssh_port,
            et_port: defaults.et_port,
            identity_file: defaults.identity_file.as_ref().map(PathBuf::from),
            jumphost: defaults.jumphost.clone(),
            user: None,
        }
    }

    pub fn apply_host_config(&mut self, host_config: &HostConfig) {
        if let Some(port) = host_config.ssh_port {
            self.ssh_port = port;
        }
        if let Some(port) = host_config.et_port {
            self.et_port = port;
        }
        if let Some(ref file) = host_config.identity_file {
            self.identity_file = Some(PathBuf::from(file));
        }
        if let Some(ref jh) = host_config.jumphost {
            self.jumphost = Some(jh.clone());
        }
        if let Some(ref user) = host_config.user {
            self.user = Some(user.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(Config::matches_pattern("*", "anything"));
        assert!(Config::matches_pattern("example.com", "example.com"));
        assert!(Config::matches_pattern("*.example.com", "foo.example.com"));
        assert!(Config::matches_pattern("*.example.com", "bar.example.com"));
        assert!(!Config::matches_pattern("*.example.com", "example.com"));
        assert!(!Config::matches_pattern("foo.com", "bar.com"));
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.defaults.ssh_port, 22);
        assert_eq!(config.defaults.et_port, 2022);
    }

    #[test]
    fn test_host_config_override() {
        let config = Config {
            defaults: Defaults {
                ssh_port: 22,
                et_port: 2022,
                identity_file: Some("~/.ssh/id_rsa".to_string()),
                jumphost: None,
                verbose: false,
            },
            hosts: vec![
                HostConfig {
                    pattern: "*.work.com".to_string(),
                    ssh_port: Some(2222),
                    et_port: None,
                    identity_file: Some("~/.ssh/work_key".to_string()),
                    jumphost: Some("bastion.work.com".to_string()),
                    user: Some("admin".to_string()),
                },
            ],
            port_forwards: vec![],
        };

        let settings = config.get_host_config("server.work.com");
        assert_eq!(settings.ssh_port, 2222);
        assert_eq!(settings.et_port, 2022);
        assert_eq!(settings.jumphost, Some("bastion.work.com".to_string()));
        assert_eq!(settings.user, Some("admin".to_string()));
    }
}
