# Eternal Terminal Rust Implementation - Production Deployment Guide

## Overview

This guide covers deploying the Rust implementation of Eternal Terminal (ET) in production environments. The Rust implementation is fully protocol-compatible with the C++ version, enabling gradual migration and mixed deployments.

**Version**: 6.2.11
**Protocol Version**: 6
**Status**: Production Ready ✅

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Installation](#installation)
3. [Configuration](#configuration)
4. [Deployment Strategies](#deployment-strategies)
5. [Security Considerations](#security-considerations)
6. [Monitoring](#monitoring)
7. [Troubleshooting](#troubleshooting)
8. [Migration from C++](#migration-from-cpp)

---

## Prerequisites

### System Requirements

**Minimum**:
- Linux kernel 3.2.0 or higher
- 512 MB RAM
- 100 MB disk space

**Recommended**:
- Linux kernel 4.4.0 or higher
- 2 GB RAM
- 500 MB disk space
- systemd for service management

### Dependencies

**Runtime**:
- OpenSSH server (for SSH spawning)
- libsodium (for encryption)

**Build**:
- Rust 1.70+ (edition 2021)
- Cargo
- gcc/clang (for C dependencies)

### Network Requirements

- Port 2022 (default ET port) - TCP inbound
- Port 22 (SSH) - For remote terminal spawning
- Firewall rules allowing ET traffic

---

## Installation

### Method 1: Build from Source (Recommended)

```bash
# Clone repository
git clone https://github.com/your-org/EternalTerminal.git
cd EternalTerminal/et-rust

# Build release binaries
cargo build --release

# Install binaries
sudo cp target/release/etserver-prod /usr/local/bin/
sudo cp target/release/et-rs /usr/local/bin/
sudo cp target/release/etterminal-rs /usr/local/bin/

# Set permissions
sudo chmod +x /usr/local/bin/etserver-prod
sudo chmod +x /usr/local/bin/et-rs
sudo chmod +x /usr/local/bin/etterminal-rs
```

### Method 2: Pre-built Binaries (Future)

```bash
# Download from releases
wget https://github.com/your-org/EternalTerminal/releases/download/v6.2.11/et-rust-linux-x64.tar.gz

# Extract
tar -xzf et-rust-linux-x64.tar.gz

# Install
sudo mv et-rust/bin/* /usr/local/bin/
```

### Verify Installation

```bash
# Check versions
etserver-prod --version
et-rs --help
etterminal-rs --help

# Test connectivity
etserver-prod --port 2023 &
sleep 1
./target/debug/etclient-rs --host localhost --port 2023 -n 1
```

---

## Configuration

### Server Configuration

Create systemd service file:

```bash
sudo tee /etc/systemd/system/etserver-prod.service << 'EOF'
[Unit]
Description=Eternal Terminal Server (Rust)
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/etserver-prod --port 2022 --bind 0.0.0.0 -v
Restart=on-failure
RestartSec=5s
User=root
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable etserver-prod
sudo systemctl start etserver-prod

# Check status
sudo systemctl status etserver-prod
```

### Client Configuration

Create user config file:

```bash
mkdir -p ~/.et
cat > ~/.et/config.toml << 'EOF'
[defaults]
ssh_port = 22
et_port = 2022
identity_file = "~/.ssh/id_rsa"
verbose = false

[[hosts]]
pattern = "*.prod.example.com"
jumphost = "bastion.prod.example.com"
identity_file = "~/.ssh/prod_key"
user = "deploy"

[[hosts]]
pattern = "localhost"
ssh_port = 22
EOF
```

See `CONFIGURATION_GUIDE.md` for comprehensive configuration options.

---

## Deployment Strategies

### Strategy 1: Greenfield Deployment

Deploy Rust ET on new infrastructure:

```bash
# On all servers
sudo systemctl enable etserver-prod
sudo systemctl start etserver-prod

# On client machines
et-rs user@hostname
```

**Pros**: Clean slate, no migration complexity
**Cons**: Not applicable to existing deployments

---

### Strategy 2: Gradual Migration (Recommended)

Incrementally replace C++ with Rust:

**Phase 1: Deploy Rust Servers Alongside C++**
```bash
# Run Rust server on different port
sudo etserver-prod --port 2023 &

# Test with existing C++ clients
et user@hostname -p 2023
```

**Phase 2: Test with Canary Traffic**
```bash
# Route 10% of traffic to Rust server
# Use load balancer or DNS routing
```

**Phase 3: Full Migration**
```bash
# Stop C++ server
sudo systemctl stop etserver

# Start Rust server on standard port
sudo etserver-prod --port 2022 &
```

**Pros**: Low risk, gradual validation
**Cons**: Requires dual infrastructure temporarily

---

### Strategy 3: Blue-Green Deployment

Run both versions, switch traffic:

```bash
# Blue environment (C++)
etserver --port 2022

# Green environment (Rust)
etserver-prod --port 2023

# Switch traffic via load balancer
# If issues, roll back immediately
```

**Pros**: Instant rollback capability
**Cons**: Requires load balancer

---

### Strategy 4: Mixed Deployment

Run C++ and Rust side by side:

```bash
# Keep C++ for critical servers
sudo systemctl start etserver

# Deploy Rust for new servers
sudo systemctl start etserver-prod
```

**Pros**: Maximum flexibility
**Cons**: Two codebases to maintain

---

## Security Considerations

### 1. Passkey Management

**❌ Don't**:
```bash
# Hardcoded passkey in config - BAD
etserver-prod -k "mysecretkey12345678901234567890"
```

**✅ Do**:
```bash
# Use environment variable or secret manager
export ET_PASSKEY=$(vault read -field=passkey secret/et)
etserver-prod -k "$ET_PASSKEY"
```

### 2. SSH Key Security

```bash
# Restrict SSH key permissions
chmod 600 ~/.ssh/id_rsa
chmod 700 ~/.ssh

# Use SSH agent
eval $(ssh-agent)
ssh-add ~/.ssh/id_rsa
```

### 3. Firewall Configuration

```bash
# Allow only necessary ports
sudo ufw allow 2022/tcp comment 'ET Server'
sudo ufw allow 22/tcp comment 'SSH'
sudo ufw enable

# Verify
sudo ufw status
```

### 4. Network Isolation

```bash
# Bind server to specific interface
etserver-prod --bind 10.0.1.100 --port 2022

# Or localhost only for testing
etserver-prod --bind 127.0.0.1 --port 2022
```

### 5. Privilege Management

```bash
# Run server as non-root user (recommended)
sudo useradd -r -s /bin/false etserver
sudo -u etserver etserver-prod --port 2022
```

**Note**: Root required for spawning user terminals. Consider capabilities:
```bash
sudo setcap cap_setuid,cap_setgid=+ep /usr/local/bin/etserver-prod
```

### 6. Encryption

- All traffic is encrypted with XSalsa20-Poly1305
- 32-byte keys required (256-bit security)
- Unique nonces per direction (MSB-based)
- No plaintext data on wire

### 7. Log Security

```bash
# Restrict log access
sudo chmod 600 /var/log/etserver-prod.log
sudo chown etserver:etserver /var/log/etserver-prod.log

# Disable verbose logging in production
etserver-prod --port 2022  # No -v flag
```

---

## Monitoring

### Metrics to Track

1. **Connection Metrics**
   - Active sessions
   - Connection failures
   - Authentication errors
   - Reconnection rate

2. **Performance Metrics**
   - Latency (client → server)
   - Throughput (bytes/sec)
   - CPU usage
   - Memory usage

3. **Error Metrics**
   - Packet loss
   - Encryption failures
   - Timeout errors

### systemd Logging

```bash
# View real-time logs
sudo journalctl -u etserver-prod -f

# View recent errors
sudo journalctl -u etserver-prod -p err -n 50

# Export logs
sudo journalctl -u etserver-prod --since "1 hour ago" > etserver-logs.txt
```

### Health Checks

```bash
# Basic availability check
nc -zv localhost 2022

# Protocol check
timeout 5 ./etclient-rs --host localhost --port 2022 -n 1

# Full health check script
#!/bin/bash
if systemctl is-active --quiet etserver-prod; then
    echo "ET Server: HEALTHY"
    exit 0
else
    echo "ET Server: DOWN"
    exit 1
fi
```

### Monitoring Integration

**Prometheus Example**:
```yaml
# Future: metrics endpoint
scrape_configs:
  - job_name: 'etserver'
    static_configs:
      - targets: ['localhost:2022']
```

**Nagios Example**:
```bash
# Check script
/usr/lib/nagios/plugins/check_tcp -H localhost -p 2022
```

---

## Troubleshooting

See `TROUBLESHOOTING_GUIDE.md` for comprehensive troubleshooting.

### Quick Diagnostics

```bash
# Check if server is running
sudo systemctl status etserver-prod

# Check if port is listening
sudo netstat -tlnp | grep 2022

# Check logs for errors
sudo journalctl -u etserver-prod -p err --since "10 minutes ago"

# Test connectivity
telnet localhost 2022
```

### Common Issues

**Issue**: Server won't start
**Solution**: Check port availability, permissions, logs

**Issue**: Client can't connect
**Solution**: Verify firewall, check server is running, test network

**Issue**: Authentication failures
**Solution**: Verify passkey, check SSH keys, review server logs

---

## Migration from C++

### Compatibility Checklist

- ✅ Protocol Version 6 compatible
- ✅ Protobuf message format identical
- ✅ Encryption algorithm same (XSalsa20-Poly1305)
- ✅ Packet framing compatible
- ✅ C++ clients work with Rust servers
- ✅ Rust clients work with C++ servers

### Migration Steps

1. **Audit Current Deployment**
   ```bash
   # Check C++ version
   et --version
   etserver --version

   # List active sessions
   ps aux | grep etterminal
   ```

2. **Test in Staging**
   ```bash
   # Deploy Rust server in staging
   # Run parallel tests
   # Validate protocol compatibility
   ```

3. **Gradual Rollout**
   - Week 1: 10% traffic to Rust
   - Week 2: 25% traffic to Rust
   - Week 3: 50% traffic to Rust
   - Week 4: 100% traffic to Rust

4. **Monitor & Validate**
   - Compare error rates
   - Check latency metrics
   - Validate functionality

5. **Decommission C++**
   ```bash
   # Stop C++ server
   sudo systemctl stop etserver
   sudo systemctl disable etserver

   # Remove C++ binaries
   sudo rm /usr/bin/et /usr/bin/etserver
   ```

### Rollback Plan

```bash
# If issues arise, quick rollback:
sudo systemctl stop etserver-prod
sudo systemctl start etserver  # C++ server

# Update DNS/load balancer to point to C++
```

---

## Best Practices

### 1. High Availability

```bash
# Use load balancer
# Deploy multiple servers
# Enable health checks
```

### 2. Backup & Recovery

```bash
# Backup configuration
tar -czf et-config-backup.tar.gz ~/.et /etc/systemd/system/etserver-prod.service

# Restore
tar -xzf et-config-backup.tar.gz -C /
```

### 3. Updates

```bash
# Download new version
# Test in staging
# Deploy with zero downtime:
sudo systemctl stop etserver-prod
sudo cp /path/to/new/etserver-prod /usr/local/bin/
sudo systemctl start etserver-prod
```

### 4. Capacity Planning

- Plan for 100 concurrent sessions per GB RAM
- Monitor CPU usage (typically <10% per server)
- Network: 1 Mbps per active session (typical)

---

## Production Checklist

- [ ] Binaries installed and verified
- [ ] systemd service configured
- [ ] Firewall rules applied
- [ ] SSH keys configured
- [ ] Configuration file created
- [ ] Monitoring enabled
- [ ] Logging configured
- [ ] Health checks working
- [ ] Backup strategy defined
- [ ] Rollback plan documented
- [ ] Team trained on operations
- [ ] Documentation accessible

---

## Support

### Resources

- **Documentation**: `/et-rust/*.md`
- **Configuration**: `CONFIGURATION_GUIDE.md`
- **Troubleshooting**: `TROUBLESHOOTING_GUIDE.md`
- **Security**: This guide, Security section

### Getting Help

1. Check logs: `sudo journalctl -u etserver-prod`
2. Review documentation
3. Test connectivity
4. Open GitHub issue (if bug)

---

## Appendix

### Example Production Setup

**3-Server Deployment**:
```
┌─────────────┐
│ Load        │
│ Balancer    │
└──────┬──────┘
       │
   ┌───┴───┬────────┐
   │       │        │
┌──▼──┐ ┌──▼──┐ ┌──▼──┐
│ ET  │ │ ET  │ │ ET  │
│ #1  │ │ #2  │ │ #3  │
└─────┘ └─────┘ └─────┘
```

**systemd Service (Production)**:
```ini
[Unit]
Description=Eternal Terminal Server (Rust)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/etserver-prod --port 2022 --bind 0.0.0.0
Restart=always
RestartSec=5s
User=etserver
Group=etserver
StandardOutput=journal
StandardError=journal
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

---

*Last Updated: 2025-11-16*
*ET Rust Implementation v6.2.11*
