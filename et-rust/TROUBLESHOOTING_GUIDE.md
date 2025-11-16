# Eternal Terminal Rust - Troubleshooting Guide

## Quick Diagnostics

### Check Server Status

```bash
# Is the server running?
sudo systemctl status etserver-prod

# Is the port listening?
sudo netstat -tlnp | grep 2022
# or
sudo ss -tlnp | grep 2022

# Check recent logs
sudo journalctl -u etserver-prod --since "10 minutes ago"

# Check for errors
sudo journalctl -u etserver-prod -p err --since "1 hour ago"
```

### Check Client Connectivity

```bash
# Can we reach the server?
telnet server.example.com 2022

# TCP connection test
nc -zv server.example.com 2022

# Full protocol test
./etclient-rs --host server.example.com --port 2022 -n 1 -v
```

---

## Common Issues

### Issue 1: Server Won't Start

**Symptoms**:
- `systemctl start etserver-prod` fails
- Service shows "failed" status

**Diagnosis**:
```bash
# Check logs
sudo journalctl -u etserver-prod -n 50

# Check port availability
sudo lsof -i :2022

# Check permissions
ls -la /usr/local/bin/etserver-prod
```

**Common Causes**:

1. **Port already in use**
   ```bash
   # Find process using port
   sudo lsof -i :2022

   # Solution: Stop conflicting process or use different port
   sudo systemctl stop etserver  # If C++ version running
   ```

2. **Permission denied**
   ```bash
   # Check binary permissions
   ls -la /usr/local/bin/etserver-prod

   # Solution: Fix permissions
   sudo chmod +x /usr/local/bin/etserver-prod
   ```

3. **Missing dependencies**
   ```bash
   # Check library dependencies
   ldd /usr/local/bin/etserver-prod

   # Solution: Install missing libraries
   sudo apt-get install libsodium23
   ```

---

### Issue 2: Client Can't Connect

**Symptoms**:
- `Connection refused`
- `Timeout`
- `No route to host`

**Diagnosis**:
```bash
# Test network connectivity
ping server.example.com

# Test port specifically
telnet server.example.com 2022

# Check firewall
sudo iptables -L -n | grep 2022
sudo ufw status | grep 2022
```

**Common Causes**:

1. **Firewall blocking**
   ```bash
   # Check firewall rules
   sudo ufw status verbose

   # Solution: Allow port
   sudo ufw allow 2022/tcp
   sudo ufw reload
   ```

2. **Server not listening on correct interface**
   ```bash
   # Check what server is listening on
   sudo netstat -tlnp | grep etserver-prod

   # If shows 127.0.0.1:2022, server is localhost-only
   # Solution: Change bind address
   etserver-prod --bind 0.0.0.0 --port 2022
   ```

3. **Network routing issue**
   ```bash
   # Trace route
   traceroute server.example.com

   # Check connectivity
   mtr server.example.com
   ```

---

### Issue 3: Authentication Failures

**Symptoms**:
- `Client is not registered`
- `Invalid key`
- `Permission denied`

**Diagnosis**:
```bash
# Check server logs
sudo journalctl -u etserver-prod | grep -i auth

# Test with verbose client
et-rs -v user@server.example.com
```

**Common Causes**:

1. **Passkey mismatch**
   ```bash
   # Server and etterminal must use same passkey
   # Check server passkey
   sudo journalctl -u etserver-prod | grep passkey

   # Solution: Ensure consistent passkey
   # Server: etserver-prod -k "$(cat /etc/et/passkey)"
   # Terminal: etterminal-rs --passkey "$(cat /etc/et/passkey)"
   ```

2. **SSH key issues**
   ```bash
   # Test SSH connection
   ssh -v user@server.example.com

   # Check key permissions
   ls -la ~/.ssh/id_rsa  # Should be 600

   # Solution: Fix permissions
   chmod 600 ~/.ssh/id_rsa
   chmod 700 ~/.ssh
   ```

3. **SSH agent not running**
   ```bash
   # Check if agent is running
   ssh-add -l

   # Solution: Start agent and add keys
   eval $(ssh-agent)
   ssh-add ~/.ssh/id_rsa
   ```

---

### Issue 4: Connection Drops / Instability

**Symptoms**:
- Connection drops after few minutes
- Intermittent disconnections
- Session hangs

**Diagnosis**:
```bash
# Check network stability
ping -c 100 server.example.com | grep loss

# Monitor connection
watch -n 1 'netstat -an | grep :2022'

# Check server logs for disconnections
sudo journalctl -u etserver-prod | grep -i disconnect
```

**Common Causes**:

1. **Network instability**
   ```bash
   # Test packet loss
   mtr -c 100 server.example.com

   # Solution: Check network path, consider different route
   ```

2. **Idle timeout**
   ```bash
   # Check for NAT/firewall timeout
   # Solution: Send keepalives (built into ET protocol)
   ```

3. **Server resource exhaustion**
   ```bash
   # Check memory
   free -h

   # Check CPU
   top -b -n 1 | grep etserver-prod

   # Solution: Increase resources or reduce load
   ```

---

### Issue 5: SSH Spawning Fails

**Symptoms**:
- `Failed to spawn etterminal on remote server`
- `SSH connection failed`
- `Remote terminal not available`

**Diagnosis**:
```bash
# Test SSH connection manually
ssh user@server.example.com "which etterminal-rs"

# Test SSH command execution
ssh user@server.example.com "echo test"

# Check SSH logs
sudo journalctl -u sshd | tail -50
```

**Common Causes**:

1. **etterminal-rs not in PATH**
   ```bash
   # Check if binary exists
   ssh user@server.example.com "which etterminal-rs"

   # Solution: Install etterminal-rs
   sudo cp target/release/etterminal-rs /usr/local/bin/
   sudo chmod +x /usr/local/bin/etterminal-rs
   ```

2. **SSH permission issues**
   ```bash
   # Test SSH
   ssh -v user@server.example.com

   # Solution: Fix SSH config, keys, permissions
   ```

3. **Remote ET server not running**
   ```bash
   # Check if server is running
   ssh user@server.example.com "systemctl status etserver-prod"

   # Solution: Start server
   ssh user@server.example.com "sudo systemctl start etserver-prod"
   ```

---

### Issue 6: Performance Issues

**Symptoms**:
- High latency
- Slow terminal response
- Packet delays

**Diagnosis**:
```bash
# Check latency
ping server.example.com

# Check bandwidth
iperf3 -c server.example.com

# Check CPU usage
top -b -n 1 | grep etserver-prod

# Check memory
free -h
```

**Common Causes**:

1. **Network latency**
   ```bash
   # Measure latency
   mtr server.example.com

   # Solution: Choose closer server, optimize route
   ```

2. **CPU/Memory exhaustion**
   ```bash
   # Check resources
   htop

   # Solution: Increase resources or reduce sessions
   ```

3. **Encryption overhead** (rare)
   ```bash
   # ET encryption is very efficient
   # If issue, consider hardware acceleration
   ```

---

### Issue 7: Protocol Compatibility Issues

**Symptoms**:
- `Protocol version mismatch`
- `Invalid packet format`
- `Decryption failed`

**Diagnosis**:
```bash
# Check versions
et-rs --help | head -1
etserver-prod --help | head -1

# Test protocol
./etclient-rs --host server.example.com --port 2022 -n 1 -v
```

**Common Causes**:

1. **Version mismatch**
   ```bash
   # Check client version
   et-rs --help

   # Check server version
   ssh server.example.com "etserver-prod --help"

   # Solution: Upgrade to same version
   ```

2. **C++ ↔ Rust compatibility issue**
   ```bash
   # Both should use Protocol Version 6
   # Check INTEROPERABILITY_RESULTS.md

   # Solution: Ensure both are v6.2.11
   ```

---

## Debugging Techniques

### Enable Verbose Logging

**Client**:
```bash
et-rs -v user@server.example.com
```

**Server**:
```bash
etserver-prod --port 2022 -v
```

### Packet Capture

```bash
# Capture ET traffic
sudo tcpdump -i eth0 -w et-traffic.pcap port 2022

# Analyze later
wireshark et-traffic.pcap
```

### strace (Advanced)

```bash
# Trace system calls
strace -f -s 200 etserver-prod --port 2022

# Trace specific process
sudo strace -p $(pgrep etserver-prod)
```

### gdb (Advanced)

```bash
# Debug binary
gdb /usr/local/bin/etserver-prod

# Attach to running process
sudo gdb -p $(pgrep etserver-prod)
```

---

## Log Analysis

### Server Logs

```bash
# View all logs
sudo journalctl -u etserver-prod

# Follow logs in real-time
sudo journalctl -u etserver-prod -f

# Filter by priority
sudo journalctl -u etserver-prod -p err

# Filter by time
sudo journalctl -u etserver-prod --since "2025-11-16 10:00"

# Export logs
sudo journalctl -u etserver-prod --since "today" > etserver-today.log
```

### Client Logs

```bash
# Run with verbose output
et-rs -v user@server.example.com 2>&1 | tee et-client.log

# Save to file
RUST_LOG=debug et-rs user@server.example.com 2> et-debug.log
```

### Log Patterns to Look For

**Errors**:
```bash
grep -i error etserver.log
grep -i fail etserver.log
grep -i denied etserver.log
```

**Warnings**:
```bash
grep -i warn etserver.log
grep -i timeout etserver.log
```

**Connection Issues**:
```bash
grep -i "connection closed" etserver.log
grep -i "connect refused" etserver.log
```

---

## Configuration Issues

### Verify Configuration

```bash
# Check config file syntax
cat ~/.et/config.toml

# Test with no config
et-rs --no-config localhost

# Verify specific host config
et-rs -v server.example.com  # Shows which config is applied
```

### Common Config Mistakes

1. **Wrong TOML syntax**
   ```toml
   # Wrong
   [defaults]
   ssh_port 22  # Missing =

   # Right
   [defaults]
   ssh_port = 22
   ```

2. **Pattern not matching**
   ```toml
   # Pattern is case-sensitive
   pattern = "*.example.com"  # Won't match Example.com
   ```

3. **Path issues**
   ```toml
   # Wrong
   identity_file = "~/ssh/id_rsa"  # ~/ may not expand

   # Right
   identity_file = "/home/user/.ssh/id_rsa"  # Absolute path
   ```

---

## Emergency Recovery

### Quick Rollback

```bash
# Stop Rust server
sudo systemctl stop etserver-prod

# Start C++ server
sudo systemctl start etserver

# Verify
ps aux | grep etserver
```

### Safe Mode Start

```bash
# Start with minimal configuration
etserver-prod --port 2022 --bind 127.0.0.1

# No daemon mode, see output directly
```

### Reset Configuration

```bash
# Backup current config
cp ~/.et/config.toml ~/.et/config.toml.backup

# Remove config
rm ~/.et/config.toml

# Test with defaults
et-rs --no-config user@server.example.com
```

---

## Getting Help

### Diagnostic Report

When requesting help, provide:

```bash
# System information
uname -a
cat /etc/os-release

# Version information
et-rs --help | head -1
etserver-prod --help | head -1

# Server status
sudo systemctl status etserver-prod

# Recent logs
sudo journalctl -u etserver-prod --since "1 hour ago" | tail -100

# Network status
sudo netstat -tlnp | grep 2022

# Configuration (sanitized)
cat ~/.et/config.toml
```

### Resources

- **Documentation**: `/et-rust/*.md`
- **Deployment Guide**: `DEPLOYMENT_GUIDE.md`
- **Configuration**: `CONFIGURATION_GUIDE.md`
- **Interoperability**: `INTEROPERABILITY_RESULTS.md`

---

## Appendix: Error Messages

### Common Error Messages and Solutions

| Error | Cause | Solution |
|-------|-------|----------|
| `Connection refused` | Server not running | Start server |
| `Permission denied (publickey)` | SSH key issue | Fix SSH keys/permissions |
| `Client is not registered` | Passkey mismatch | Align passkeys |
| `Protocol version mismatch` | Version incompatibility | Upgrade to same version |
| `Failed to decrypt packet` | Wrong passkey | Use correct passkey |
| `Port already in use` | Port conflict | Stop conflicting process |
| `No route to host` | Network issue | Check network/firewall |
| `Timeout` | Network or server issue | Check connectivity |

---

*Last Updated: 2025-11-16*
*ET Rust Implementation v6.2.11*
