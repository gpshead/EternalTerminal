# Eternal Terminal Configuration Guide

## Overview

Eternal Terminal supports configuration files for persistent settings and per-host customization. This eliminates the need to specify common options on every invocation.

## Configuration Locations

ET searches for configuration files in the following order:

1. **User Config**: `~/.et/config.toml` (preferred for per-user settings)
2. **System Config**: `/etc/et/config.toml` (for system-wide defaults)

User config takes precedence over system config.

## Configuration Format

ET uses TOML format for configuration files. See `config.toml.example` for a complete example.

### Basic Structure

```toml
[defaults]
ssh_port = 22
et_port = 2022

[[hosts]]
pattern = "*.example.com"
ssh_port = 2222
```

## Configuration Sections

### 1. Defaults Section

Global defaults applied to all connections:

```toml
[defaults]
ssh_port = 22           # Default SSH port
et_port = 2022          # Default ET server port
identity_file = "~/.ssh/id_rsa"  # Default SSH key
jumphost = "bastion.example.com" # Default jumphost
verbose = false         # Enable verbose logging
```

All fields are optional. If not specified, hardcoded defaults are used.

### 2. Host-Specific Configuration

Define per-host or per-pattern settings:

```toml
[[hosts]]
pattern = "*.work.com"
ssh_port = 2222
identity_file = "~/.ssh/work_key"
jumphost = "bastion.work.com"
user = "admin"
```

**Fields**:
- `pattern` (required): Hostname pattern with wildcard support
- `ssh_port`: SSH port override
- `et_port`: ET server port override
- `identity_file`: SSH private key path
- `jumphost`: Intermediate SSH host
- `user`: SSH username override

**Pattern Matching**:
- Exact: `"server.example.com"` matches only "server.example.com"
- Wildcard: `"*.example.com"` matches "foo.example.com", "bar.example.com"
- Prefix: `"prod-*"` matches "prod-web", "prod-db"
- All: `"*"` matches everything

**Precedence**: Later matches override earlier matches. Place more specific patterns after general ones.

### 3. Port Forwarding Rules

Define port forwards to be established automatically:

```toml
[[port_forwards]]
local_port = 8080
remote_host = "localhost"
remote_port = 80
for_host = "web-server.example.com"  # Optional: apply only to this host
```

**Note**: Port forwarding implementation is planned for future phases. The configuration structure is defined now for forward compatibility.

## Command-Line Override

Command-line arguments always override configuration file settings:

```bash
# Config says ssh_port=2222, but this uses port 22
et -ssh-port 22 server.example.com

# Config says identity_file="~/.ssh/default", but this uses special key
et -i ~/.ssh/special_key server.example.com
```

## Configuration Priority

Settings are resolved in this order (highest priority first):

1. **Command-line arguments** (e.g., `-i`, `--ssh-port`)
2. **Target specification** (e.g., `user@host:port`)
3. **User config file** (`~/.et/config.toml`)
4. **System config file** (`/etc/et/config.toml`)
5. **Hardcoded defaults**

## Example Configurations

### Example 1: Work Environment

```toml
# ~/.et/config.toml
[defaults]
identity_file = "~/.ssh/id_rsa"

[[hosts]]
pattern = "*.corp.example.com"
ssh_port = 2222
jumphost = "bastion.corp.example.com"
user = "jdoe"

[[hosts]]
pattern = "prod-*"
identity_file = "~/.ssh/production_key"
user = "deploy"
```

Usage:
```bash
# Uses bastion jumphost, port 2222, user jdoe
et server.corp.example.com

# Uses production key, user deploy
et prod-web-01
```

### Example 2: Development Setup

```toml
[defaults]
et_port = 2022
verbose = true

[[hosts]]
pattern = "localhost"
ssh_port = 22

[[hosts]]
pattern = "*.dev"
ssh_port = 22
user = "developer"
et_port = 3000  # Dev servers run ET on different port
```

### Example 3: Multi-Environment

```toml
# System-wide config: /etc/et/config.toml
[defaults]
et_port = 2022

[[hosts]]
pattern = "*.staging.example.com"
jumphost = "staging-bastion.example.com"

[[hosts]]
pattern = "*.production.example.com"
jumphost = "prod-bastion.example.com"
identity_file = "/etc/keys/production_key"
```

## Configuration Validation

ET validates configuration on load:

```bash
# Test configuration loading
et --no-config localhost  # Skip config file
et -v localhost           # Verbose mode shows config loading
```

If configuration fails to parse, ET falls back to defaults and logs a warning.

## Tips and Best Practices

1. **Use patterns wisely**: Group similar hosts with wildcards instead of duplicating configuration.

2. **Keep sensitive keys in user config**: Store production keys in user-specific config, not system-wide.

3. **Test with --no-config**: Verify behavior without config using `--no-config` flag.

4. **Use comments**: Document why specific settings exist, especially for unusual configurations.

5. **Version control safe**: Exclude `~/.et/config.toml` from version control if it contains sensitive paths.

## Troubleshooting

### Config not loading

```bash
# Check if config file exists
ls -la ~/.et/config.toml

# Test with verbose logging
et -v localhost
```

### Wrong settings being applied

```bash
# Check precedence: CLI > target > user config > system config
et -v your-host  # Shows which settings are applied
```

### Pattern not matching

- Patterns are case-sensitive
- Use exact hostname as shown in connection attempts
- Test patterns: `"*"` matches everything

## Migration from SSH Config

Many ET users also use `~/.ssh/config`. Here's how they compare:

| Feature | SSH Config | ET Config |
|---------|------------|-----------|
| Hostname patterns | ✓ | ✓ |
| Port specification | ✓ | ✓ (ssh_port + et_port) |
| Identity file | ✓ | ✓ |
| Jumphost (ProxyJump) | ✓ | ✓ (single hop) |
| Port forwarding | ✓ | Planned |
| Local config | ~/.ssh/config | ~/.et/config.toml |
| System config | /etc/ssh/ssh_config | /etc/et/config.toml |

You can use both simultaneously. ET config is specific to ET connections, while SSH config applies to all SSH operations (including ET's SSH usage).

## Future Enhancements

Planned for future phases:

- **Port forwarding implementation**: Automatic tunnel establishment
- **Multi-hop jumphosts**: ProxyJump-style chaining
- **Dynamic port allocation**: Auto-assign local ports
- **Config profiles**: Named configuration sets
- **Environment variable expansion**: More flexible path specification
