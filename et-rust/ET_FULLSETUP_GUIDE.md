# Eternal Terminal - Full Setup Guide for Non-Root Users

This guide documents how to set up and test Eternal Terminal as a non-root user with only localhost access.

## Overview

Eternal Terminal (ET) provides persistent remote shell sessions that survive network interruptions. The architecture consists of:

1. **SSH Server**: Standard OpenSSH daemon for initial authentication
2. **ET Server** (`etserver`): Daemon listening on port 2022 (default)
3. **ET Client** (`et`): Client binary that connects to remote ET servers
4. **ET Terminal** (`etterminal`): Terminal multiplexer spawned by SSH

## Architecture Flow

```
[ET Client] ---(ET Protocol Port 2022)---> [ET Server]
                                              |
                                              v
[ET Client] ---(SSH Port 22)-----------> [SSH Server]
                                              |
                                              v
                                         [etterminal]
                                              |
                                              v
                                        [User Shell]
```

**Connection Sequence:**
1. ET client connects to ET server on port 2022
2. Client authenticates via SSH (tunneled or separate)
3. Server spawns `etterminal` via SSH
4. `etterminal` manages the persistent terminal session
5. ET protocol handles reconnection and data synchronization

## Prerequisites

### System Requirements
- Linux system with network access
- OpenSSH server and client installed
- Build tools for compiling ET (or pre-built binaries)

### User Permissions
- Non-privileged user account
- Ability to bind to non-privileged ports (>1024)
- SSH access to localhost (or remote server)

## Setup Steps

### 1. Install SSH Server

```bash
# Install OpenSSH server and client
sudo apt-get update
sudo apt-get install -y openssh-server openssh-client

# Configure SSH to bind only to localhost (for security)
echo "ListenAddress 127.0.0.1" | sudo tee -a /etc/ssh/sshd_config

# Start SSH server
sudo mkdir -p /var/run/sshd
sudo /usr/sbin/sshd

# Verify SSH is running and bound to localhost only
ps aux | grep sshd | grep -v grep
sudo netstat -tlnp | grep :22 || sudo ss -tlnp | grep :22
```

**Security Note**: By binding SSH to `127.0.0.1` only, the SSH server will only accept connections from localhost, preventing external access.

### 2. Set Up SSH Keys for Passwordless Authentication

```bash
# Generate SSH key (as your user)
ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -N '' -C "user@localhost"

# Add public key to authorized_keys
cat ~/.ssh/id_ed25519.pub >> ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys

# Test SSH connection
ssh -o StrictHostKeyChecking=no localhost 'echo SSH works && whoami'
```

**Expected output:**
```
SSH works
<your_username>
```

### 3. Build or Install ET Binaries

#### Option A: Build from Source

```bash
cd /path/to/EternalTerminal

# Install dependencies
sudo apt-get install -y build-essential cmake git pkg-config libssl-dev \
    libsodium-dev libprotobuf-dev protobuf-compiler zlib1g-dev \
    libcurl4-openssl-dev

# Initialize submodules
git submodule update --init --recursive

# Build
mkdir -p build && cd build
cmake -DDISABLE_VCPKG=ON ..
make -j4 et etserver etterminal

# Binaries created:
# - build/et (54M) - Client
# - build/etserver (55M) - Server
# - build/etterminal (38M) - Terminal multiplexer
```

#### Option B: Use Pre-built Binaries

```bash
# Copy binaries to user directory
mkdir -p ~/bin
cp /path/to/build/{et,etserver,etterminal} ~/bin/
chmod +x ~/bin/*

# Add to PATH
echo 'export PATH=~/bin:$PATH' >> ~/.bashrc
source ~/.bashrc
```

### 4. Configure Logging Directory

ET requires a log directory to be specified or it will try to use system paths:

```bash
mkdir -p ~/et-logs
```

### 5. Start ET Server

```bash
# Start etserver on non-privileged port 2022, bound to localhost only
etserver --port 2022 \
         --bindip 127.0.0.1 \
         --pidfile ~/etserver.pid \
         --logdir ~/et-logs \
         --logtostdout &

# Verify server is running and bound to localhost only
ps aux | grep etserver | grep -v grep
netstat -tlnp 2>/dev/null | grep :2022 || ss -tlnp 2>/dev/null | grep :2022
```

**Expected log output:**
```
[INFO] In child, about to start server.
[INFO] Listening on 127.0.0.1:2022/2/1/6
[INFO] Listening on 127.0.0.1:2022/10/1/6
[INFO] Creating server
```

**Security Note**: The `--bindip 127.0.0.1` parameter ensures ET server only accepts connections from localhost, preventing external access. This is critical for localhost-only testing environments.

### 6. Connect with ET Client

#### Basic Connection

```bash
et --terminal-path ~/bin/etterminal \
   --logdir ~/et-logs \
   username@localhost
```

#### Connection with Command Execution

```bash
et --terminal-path ~/bin/etterminal \
   --logdir ~/et-logs \
   -c 'echo Hello && whoami' \
   username@localhost
```

#### Connection Parameters

**Server Parameters:**
| Parameter | Description | Default |
|-----------|-------------|---------|
| `--port` | ET server port | 2022 |
| `--bindip` | IP address to bind to | "" (all interfaces) |
| `--logdir` | Directory for log files | Required |
| `--pidfile` | Location of PID file | /var/run/etserver.pid |
| `--logtostdout` | Log to stdout instead of file | false |

**Client Parameters:**
| Parameter | Description | Default |
|-----------|-------------|---------|
| `--port` | ET server port to connect to | 2022 |
| `--terminal-path` | Path to etterminal binary | (searches PATH) |
| `--logdir` | Directory for log files | Required |
| `--logtostdout` | Log to stdout instead of file | false |
| `-c, --command` | Command to run and exit | (interactive) |
| `-t, --tunnel` | Port forwarding (local:remote) | none |

## Testing the Setup

### Test 1: SSH Connectivity

```bash
ssh localhost 'echo Test successful'
```

✅ **Success**: "Test successful" printed
❌ **Failure**: Check SSH server status and keys

### Test 2: ET Server Listening

```bash
# Check if etserver is bound to localhost only:
netstat -tlnp 2>/dev/null | grep 2022 || ss -tlnp 2>/dev/null | grep 2022
# Or:
lsof -i :2022 2>/dev/null
```

✅ **Success**: etserver listening on `127.0.0.1:2022` (localhost only)
❌ **Failure**: Check server logs in ~/et-logs

**Note**: If you see `0.0.0.0:2022`, the server is bound to all interfaces (less secure). Restart with `--bindip 127.0.0.1` to bind to localhost only.

### Test 3: ET Client Connection

```bash
et --terminal-path ~/bin/etterminal \
   --logdir ~/et-logs \
   -c 'echo ET connection works!' \
   $USER@localhost
```

✅ **Success**: "ET connection works!" printed
❌ **Failure**: Check error messages and logs

### Test 4: Persistent Connection

```bash
# Terminal 1: Start ET session
et --terminal-path ~/bin/etterminal \
   --logdir ~/et-logs \
   $USER@localhost

# Inside ET session, start a long-running process
for i in {1..100}; do echo "Iteration $i"; sleep 1; done

# Simulate network interruption (in another terminal)
# The session should reconnect automatically
```

## Troubleshooting

### Issue: "etterminal: command not found"

**Cause**: etterminal is not in PATH or not specified

**Solution**:
```bash
# Option 1: Add to PATH
echo 'export PATH=~/bin:$PATH' >> ~/.bashrc
source ~/.bashrc

# Option 2: Use --terminal-path
et --terminal-path ~/bin/etterminal user@host
```

### Issue: "Error: (22): Invalid argument" on startup

**Cause**: Missing or inaccessible log directory

**Solution**:
```bash
# Create log directory
mkdir -p ~/et-logs

# Always specify --logdir and bind to localhost
etserver --port 2022 \
         --bindip 127.0.0.1 \
         --logdir ~/et-logs \
         --pidfile ~/etserver.pid
```

### Issue: "Address already in use (os error 98)"

**Cause**: Another process is using port 2022

**Solution**:
```bash
# Find and kill existing etserver
pkill -f etserver

# Or use a different port (still bind to localhost)
etserver --port 2023 \
         --bindip 127.0.0.1 \
         --logdir ~/et-logs \
         --pidfile ~/etserver.pid
et --port 2023 user@localhost
```

### Issue: "Error handling new client: Failed a call to readAll"

**Cause**: Protocol handshake failure between client and server

**Potential causes**:
- Version mismatch between client and server
- Network interruption during handshake
- SSH authentication failure

**Solution**:
1. Ensure client and server binaries are from the same build
2. Check SSH connectivity: `ssh localhost echo test`
3. Check server logs for detailed error information
4. Try with verbose logging: `-v 9`

### Issue: SSH connection refused

**Cause**: SSH server not running or not listening on expected port

**Solution**:
```bash
# Check if SSH server is running
sudo systemctl status ssh
# Or:
ps aux | grep sshd

# Start SSH server if needed
sudo systemctl start ssh
# Or:
sudo /usr/sbin/sshd
```

## Configuration Files

### ~/.ssh/config (Optional)

```
Host myserver
    HostName localhost
    User myuser
    Port 22
    IdentityFile ~/.ssh/id_ed25519
```

Then connect with:
```bash
et --terminal-path ~/bin/etterminal --logdir ~/et-logs myserver
```

### ET Server Systemd Service (Optional - requires root)

```ini
[Unit]
Description=Eternal Terminal Server
After=network.target

[Service]
Type=forking
User=myuser
ExecStart=/home/myuser/bin/etserver \
          --daemon \
          --port 2022 \
          --bindip 127.0.0.1 \
          --logdir /home/myuser/et-logs \
          --pidfile /home/myuser/etserver.pid

[Install]
WantedBy=multi-user.target
```

**Note**: For remote access, change `--bindip 127.0.0.1` to `--bindip 0.0.0.0` or specify your server's IP address.

## Security Considerations

### For Non-Root Users

1. **Network Binding**: Always bind to localhost for local-only access
   - **Localhost only** (recommended for testing): `--bindip 127.0.0.1` ✅
   - **All interfaces** (external access): `--bindip 0.0.0.0` ⚠️ Only if needed
   - **Specific interface**: `--bindip <your-ip>` for controlled access

   ```bash
   # Secure (localhost only)
   etserver --port 2022 --bindip 127.0.0.1 --logdir ~/et-logs --pidfile ~/etserver.pid

   # Less secure (all interfaces) - only use if you need remote access
   etserver --port 2022 --bindip 0.0.0.0 --logdir ~/et-logs --pidfile ~/etserver.pid
   ```

2. **SSH Server Binding**: Configure SSH to bind to localhost only
   ```bash
   # Add to /etc/ssh/sshd_config
   ListenAddress 127.0.0.1

   # For IPv6 localhost:
   ListenAddress ::1
   ```

3. **Port Selection**: Use ports >1024 (non-privileged)
   - Default ET port: 2022 ✅
   - Privileged ports (<1024): ❌ Require root

4. **File Permissions**:
   ```bash
   chmod 700 ~/.ssh
   chmod 600 ~/.ssh/authorized_keys
   chmod 600 ~/.ssh/id_ed25519
   chmod 644 ~/.ssh/id_ed25519.pub
   ```

5. **Firewall Rules**: If running on a server accessible from internet
   ```bash
   # Allow only specific IPs (if needed)
   sudo ufw allow from 192.168.1.0/24 to any port 2022

   # Or block all external access to ET port
   sudo ufw deny 2022
   ```

6. **SSH Hardening**:
   - Disable password authentication (use keys only)
   - Use strong key types (ed25519, rsa 4096+)
   - Keep SSH server updated
   - Restrict SSH to localhost if not needed externally

## Performance Tuning

### Connection Keepalive

ET has built-in keepalive mechanisms, but you can also configure TCP keepalive:

Add to `~/.ssh/config`:
```
ServerAliveInterval 60
ServerAliveCountMax 3
```

### Log Rotation

```bash
# Create logrotate config (requires root)
sudo tee /etc/logrotate.d/eternalterminal <<EOF
/home/*/et-logs/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
}
EOF
```

## Comparison with Standard SSH

| Feature | SSH | Eternal Terminal |
|---------|-----|------------------|
| Survives network interruption | ❌ No | ✅ Yes |
| Reconnection | Manual | Automatic |
| Port forwarding | ✅ Yes | ✅ Yes |
| Scrollback buffer | Client-dependent | Synchronized |
| Authentication | SSH keys/password | SSH (delegated) |
| Encryption | SSH protocol | libsodium (XSalsa20-Poly1305) |
| Latency sensitivity | High | Low (buffered) |

## Advanced Usage

### Port Forwarding

```bash
# Forward local port 8080 to remote port 80
et -t 8080:80 user@remote-server

# Forward multiple ports
et -t 8080:80,8443:443 user@remote-server

# Reverse tunnel (remote port to local)
et -r 8080:80 user@remote-server
```

### Jumphost

```bash
# Connect through a jumphost
et --jumphost jump.example.com \
   --jport 2022 \
   user@internal-server
```

### Kill Other Sessions

```bash
# Kill all other ET sessions for this user
et -x user@server
```

## Rust Implementation Notes

The Rust port of ET (`etclient-rs` and `etserver-rs`) implements the core protocol but not the full terminal features:

### What's Implemented (Rust)
- ✅ Protocol version 6 handshake
- ✅ XSalsa20-Poly1305 encryption
- ✅ Packet serialization/deserialization
- ✅ Backed buffer for retransmission (64MB)
- ✅ Connection management with auto-reconnect
- ✅ 21 passing unit tests

### What's NOT Implemented (Rust)
- ❌ Terminal/PTY handling
- ❌ SSH integration
- ❌ Port forwarding
- ❌ Jumphost support
- ❌ Session management

The Rust implementation serves as a protocol validation tool, demonstrating wire-level compatibility with the C++ implementation.

## Summary

Eternal Terminal provides resilient remote shell sessions ideal for:
- ✅ Unreliable network connections (WiFi, mobile, VPN)
- ✅ Long-running processes that must survive disconnects
- ✅ Low-latency requirements with network buffering

**Setup Checklist:**
- [ ] SSH server running and bound to localhost (127.0.0.1)
- [ ] SSH keys configured for passwordless auth
- [ ] ET binaries (et, etserver, etterminal) in PATH
- [ ] Log directory created
- [ ] etserver started on port 2022, bound to localhost
- [ ] ET client successfully connects
- [ ] Persistent connection tested through interruption

**Quick Start (Localhost Testing):**
```bash
# Server side (bound to localhost for security)
etserver --port 2022 \
         --bindip 127.0.0.1 \
         --logdir ~/et-logs \
         --pidfile ~/etserver.pid

# Client side
et --terminal-path ~/bin/etterminal --logdir ~/et-logs user@localhost
```

**Quick Start (Remote Access):**
```bash
# Server side (bind to all interfaces or specific IP)
etserver --port 2022 \
         --bindip 0.0.0.0 \
         --logdir ~/et-logs \
         --pidfile ~/etserver.pid

# Client side (from remote machine)
et --terminal-path ~/bin/etterminal --logdir ~/et-logs user@remote-server
```

For more information, see:
- Official docs: https://github.com/MisterTea/EternalTerminal
- Rust port status: `RUST_PORT_STATUS.md`
- Protocol details: `et-rust/et-proto/`
