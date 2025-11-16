# Eternal Terminal Rust - Security Guide

## Overview

This document describes the security architecture, considerations, and best practices for the Eternal Terminal (ET) Rust implementation. ET is designed with security as a core principle, providing encrypted, authenticated connections for persistent terminal sessions.

**Version**: 6.2.11
**Last Updated**: 2025-11-16

---

## Table of Contents

1. [Security Architecture](#security-architecture)
2. [Threat Model](#threat-model)
3. [Cryptographic Design](#cryptographic-design)
4. [Authentication](#authentication)
5. [Network Security](#network-security)
6. [Access Control](#access-control)
7. [Key Management](#key-management)
8. [Deployment Security](#deployment-security)
9. [Monitoring and Auditing](#monitoring-and-auditing)
10. [Security Best Practices](#security-best-practices)
11. [Vulnerability Reporting](#vulnerability-reporting)
12. [Security Checklist](#security-checklist)

---

## Security Architecture

### Defense in Depth

ET implements multiple layers of security:

```
┌─────────────────────────────────────────────────┐
│         Application Layer (Terminal I/O)        │
├─────────────────────────────────────────────────┤
│       Protocol Layer (ET Protocol v6)            │
│  - Authenticated Encryption (XSalsa20-Poly1305) │
│  - Message Authentication (MAC)                  │
│  - Nonce-based Protection                        │
├─────────────────────────────────────────────────┤
│         Transport Layer (TCP)                    │
│  - Connection-oriented                           │
│  - Port-based Access Control                     │
├─────────────────────────────────────────────────┤
│         SSH Layer (Initial Connection)           │
│  - Public Key Authentication                     │
│  - Host Key Verification                         │
│  - Encrypted Channel                             │
├─────────────────────────────────────────────────┤
│         Network Layer (Firewall)                 │
│  - IP-based Filtering                            │
│  - Port Restrictions                             │
└─────────────────────────────────────────────────┘
```

### Trust Boundaries

1. **Client → SSH**: Trusted (authenticated via SSH keys)
2. **SSH → etterminal-rs**: Trusted (spawned by authenticated SSH session)
3. **etterminal-rs → etserver-prod**: Authenticated (passkey + encryption)
4. **etserver-prod → PTY**: Trusted (server-controlled)

---

## Threat Model

### Assets to Protect

1. **Terminal Sessions**: User commands, outputs, sensitive data
2. **Credentials**: SSH keys, passkeys, authentication tokens
3. **Server Resources**: CPU, memory, file descriptors
4. **Network Bandwidth**: Protection against DoS

### Threat Actors

1. **Network Attackers**: Eavesdropping, MitM, packet injection
2. **Unauthorized Users**: Attempting to connect without credentials
3. **Malicious Clients**: Attempting to exhaust server resources
4. **Compromised Clients**: Pivoting to attack server

### Attack Vectors

#### 1. Network Attacks

**Threat**: Eavesdropping on terminal traffic

**Mitigation**:
- ✅ All ET protocol traffic encrypted (XSalsa20-Poly1305)
- ✅ Initial SSH connection encrypted
- ✅ No plaintext transmission of sensitive data

**Threat**: Man-in-the-Middle (MitM) attacks

**Mitigation**:
- ✅ SSH host key verification (initial connection)
- ✅ Authenticated encryption (Poly1305 MAC)
- ✅ Nonce-based replay protection

**Threat**: Packet injection or modification

**Mitigation**:
- ✅ Message Authentication Codes (MAC) detect tampering
- ✅ Packets fail decryption if modified
- ✅ Sequence numbers prevent reordering attacks

#### 2. Authentication Attacks

**Threat**: Unauthorized access attempts

**Mitigation**:
- ✅ Passkey required for ET protocol connection
- ✅ SSH public key authentication (initial connection)
- ✅ Failed authentication attempts logged

**Threat**: Passkey brute force

**Mitigation**:
- ⚠️ Use strong passkeys (256-bit recommended)
- ✅ Rate limiting via TCP connection overhead
- ✅ Monitor failed connection attempts

#### 3. Denial of Service (DoS)

**Threat**: Connection exhaustion

**Mitigation**:
- ✅ Tokio async runtime handles many concurrent connections
- ✅ Server can limit connections (implementation detail)
- 🔲 Rate limiting (future enhancement)

**Threat**: Memory exhaustion

**Mitigation**:
- ✅ Rust memory safety prevents overflows
- ✅ Bounded buffers for packet processing
- ✅ Resource cleanup on connection close

#### 4. Privilege Escalation

**Threat**: Gaining elevated privileges

**Mitigation**:
- ✅ etterminal-rs runs with user privileges (not root)
- ✅ etserver-prod can run as non-root (recommended)
- ✅ PTY permissions match user permissions

---

## Cryptographic Design

### Encryption Algorithm: XSalsa20-Poly1305

**Choice Rationale**:
- **XSalsa20**: Stream cipher, fast, constant-time
- **Poly1305**: Message Authentication Code (MAC)
- **Combined**: Authenticated Encryption with Associated Data (AEAD)

**Security Properties**:
- **Confidentiality**: Ciphertext reveals no information about plaintext
- **Authenticity**: MAC prevents tampering
- **Integrity**: Any modification detected
- **Forward Secrecy**: Not applicable (symmetric key)

**Implementation**:
```rust
use crypto_box::aead::{Aead, AeadCore, KeyInit};
use crypto_box::XSalsa20Poly1305;

// Encrypt packet
let cipher = XSalsa20Poly1305::new(key.into());
let nonce = XSalsa20Poly1305::generate_nonce(&mut OsRng);
let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref())?;
```

**Nonce Generation**:
- 24-byte nonces (192 bits)
- Directional MSB to prevent nonce reuse:
  - Client → Server: MSB = `CLIENT_SERVER_NONCE_MSB` (0)
  - Server → Client: MSB = `SERVER_CLIENT_NONCE_MSB` (1)
- Counter-based, incremented per packet
- No nonce reuse within same connection

### Key Derivation

**Passkey Handling**:
```rust
// Passkey → Encryption Key
let key = if passkey.len() >= 32 {
    passkey[..32].to_vec()  // Use first 32 bytes
} else {
    // Pad with zeros (for compatibility with C++)
    let mut padded = passkey.to_vec();
    padded.resize(32, 0);
    padded
};
```

**Security Notes**:
- ⚠️ Passkey should be 32 bytes (256 bits) for maximum security
- ⚠️ Weak passkeys reduce security to brute-force resistance
- ✅ Use cryptographically random passkeys

**Recommended Passkey Generation**:
```bash
# Generate strong passkey (256 bits)
openssl rand -base64 32

# Or using /dev/urandom
head -c 32 /dev/urandom | base64

# Store securely
echo "$(openssl rand -base64 32)" > /etc/et/passkey
chmod 600 /etc/et/passkey
```

### Packet Format

```
┌───────────────────────────────────────────┐
│  4 bytes: Packet Length (big-endian)      │
├───────────────────────────────────────────┤
│  N bytes: Encrypted Protobuf Message      │
│  (Encrypted with XSalsa20-Poly1305)       │
│  [Includes Poly1305 MAC for auth]         │
└───────────────────────────────────────────┘
```

**Security Properties**:
- Length prefix prevents buffer overflows
- Encryption prevents content disclosure
- MAC prevents tampering
- No unauthenticated data

---

## Authentication

### Two-Layer Authentication

#### Layer 1: SSH Authentication (Initial Connection)

**Purpose**: Authenticate user to server for spawning etterminal-rs

**Methods**:
1. **Public Key Authentication** (recommended)
   - Uses ~/.ssh/id_rsa or ~/.ssh/id_ed25519
   - Host key verification prevents MitM
   - No password transmission over network

2. **Password Authentication** (not recommended)
   - ⚠️ Subject to brute force
   - ⚠️ Password transmitted (encrypted by SSH)
   - ✅ Still protected by SSH encryption

**Configuration**:
```bash
# Generate SSH key
ssh-keygen -t ed25519 -C "user@client"

# Copy to server
ssh-copy-id user@server

# Verify
ssh user@server "echo 'SSH works'"
```

#### Layer 2: ET Protocol Authentication (Ongoing Connection)

**Purpose**: Authenticate etterminal-rs to etserver-prod

**Method**: Shared passkey

**Security Model**:
- etterminal-rs and etserver-prod share a secret passkey
- Passkey is used as encryption key
- Successful decryption implies authentication
- No separate authentication protocol (implicit auth)

**Passkey Distribution**:
```bash
# Server side: /etc/et/passkey
$ cat /etc/et/passkey
<32-byte random passkey>

# etterminal-rs: Reads from server environment
$ etterminal-rs --passkey "$(cat /etc/et/passkey)"

# Client side: Passed via SSH environment
$ et-rs localhost  # Client uses default or config passkey
```

**Security Considerations**:
- ✅ Passkey never transmitted over network
- ✅ Passkey only in server memory (etterminal + etserver)
- ⚠️ Passkey stored on disk (should be protected)
- ⚠️ All clients use same passkey (no per-user keys)

### Client Identification

Each ET session has a unique `client_id`:

```rust
pub struct Session {
    client_id: String,  // Unique identifier
    passkey: Vec<u8>,   // Encryption key
    socket: Box<dyn AsyncSocket>,
}
```

**Purpose**:
- Session tracking
- Logging and auditing
- Future: Session resume

**Security Note**:
- client_id is not secret
- client_id does not provide authentication
- Authentication is via passkey/encryption

---

## Network Security

### Port Configuration

**Default Ports**:
- SSH: 22
- ET Server: 2022

**Firewall Rules**:

```bash
# Allow SSH (for initial connection)
sudo ufw allow 22/tcp

# Allow ET Server (only from trusted networks)
sudo ufw allow from 10.0.0.0/8 to any port 2022 proto tcp

# Or allow from specific IPs
sudo ufw allow from 192.168.1.0/24 to any port 2022 proto tcp

# Enable firewall
sudo ufw enable

# Verify
sudo ufw status verbose
```

**Best Practices**:
- ✅ Restrict ET server port to trusted networks
- ✅ Use VPN or bastion hosts for public access
- ✅ Do not expose ET server port directly to internet
- ✅ Monitor connection attempts

### Bind Address Configuration

**Server Binding**:

```bash
# Localhost only (most secure, for local testing)
etserver-prod --port 2022 --bind 127.0.0.1

# Specific interface
etserver-prod --port 2022 --bind 192.168.1.10

# All interfaces (less secure, requires firewall)
etserver-prod --port 2022 --bind 0.0.0.0
```

**Security Implications**:
- `127.0.0.1`: Only local connections (most secure)
- Specific IP: Only connections to that interface
- `0.0.0.0`: Connections from any interface (requires firewall)

### Network Encryption

**Data at Rest**: Not encrypted (terminal output is ephemeral)

**Data in Transit**:
1. **SSH phase**: Encrypted by SSH
2. **ET protocol phase**: Encrypted by XSalsa20-Poly1305

**Metadata Protection**:
- ❌ Packet sizes visible (traffic analysis possible)
- ❌ Connection timing visible
- ❌ Destination server visible

**Mitigation**:
- Use VPN for additional traffic obfuscation
- Use Tor for anonymity (not recommended for latency reasons)

---

## Access Control

### File System Permissions

**Server Binary**:
```bash
# etserver-prod should be executable by root or dedicated user
sudo chown root:et /usr/local/bin/etserver-prod
sudo chmod 750 /usr/local/bin/etserver-prod
```

**Passkey File**:
```bash
# Passkey should be readable only by server user
sudo chown root:root /etc/et/passkey
sudo chmod 600 /etc/et/passkey
```

**etterminal Binary**:
```bash
# Should be executable by all users
sudo chown root:root /usr/local/bin/etterminal-rs
sudo chmod 755 /usr/local/bin/etterminal-rs
```

### User Permissions

**etterminal-rs**:
- Runs with privileges of SSH user
- Creates PTY owned by SSH user
- No privilege escalation

**etserver-prod**:
- Can run as root (for port 2022 if < 1024)
- Recommended: Run as dedicated user with CAP_NET_BIND_SERVICE
- Does not require root after binding port

**Privilege Dropping** (future enhancement):
```bash
# Start as root, drop privileges after binding port
etserver-prod --port 2022 --user etserver --group etserver
```

### SELinux / AppArmor

**SELinux Considerations**:
- ET binaries may need policy for network binding
- PTY allocation requires appropriate permissions
- SSH spawning requires appropriate transitions

**Example SELinux Policy** (conceptual):
```
# Allow ET server to bind ports
allow etserver_t self:tcp_socket { bind listen accept };
allow etserver_t et_port_t:tcp_socket { name_bind };

# Allow spawning PTY
allow etserver_t devpts_t:chr_file { open read write };
```

---

## Key Management

### Passkey Lifecycle

#### 1. Generation

**Recommended**:
```bash
# Generate strong random passkey
openssl rand -base64 32 > /etc/et/passkey
chmod 600 /etc/et/passkey
```

**Avoid**:
- ❌ Weak passkeys (dictionary words, short strings)
- ❌ Reusing passkeys across environments
- ❌ Storing passkeys in version control

#### 2. Distribution

**Server Side**:
```bash
# Store in protected location
sudo mkdir -p /etc/et
sudo bash -c 'openssl rand -base64 32 > /etc/et/passkey'
sudo chmod 600 /etc/et/passkey
```

**Client Side** (for etterminal-rs):
- Passkey should already be on server (same machine)
- etterminal-rs reads from /etc/et/passkey or receives via environment

**Automation** (Ansible example):
```yaml
- name: Deploy ET passkey
  copy:
    content: "{{ et_passkey }}"
    dest: /etc/et/passkey
    owner: root
    group: root
    mode: '0600'
  no_log: true  # Don't log passkey value
```

#### 3. Rotation

**Current Implementation**: No automatic rotation

**Manual Rotation**:
```bash
# 1. Generate new passkey
openssl rand -base64 32 > /etc/et/passkey.new

# 2. Update server configuration
sudo systemctl stop etserver-prod
sudo mv /etc/et/passkey.new /etc/et/passkey
sudo systemctl start etserver-prod

# 3. Restart existing sessions
# (existing sessions will break, reconnect with new passkey)
```

**Best Practices**:
- Rotate passkeys regularly (e.g., every 90 days)
- Rotate immediately if compromise suspected
- Coordinate rotation to minimize disruption

#### 4. Revocation

**When to Revoke**:
- Compromise suspected
- Employee termination
- Security audit finding

**How to Revoke**:
```bash
# 1. Stop server
sudo systemctl stop etserver-prod

# 2. Generate new passkey
openssl rand -base64 32 > /etc/et/passkey

# 3. Restart server
sudo systemctl start etserver-prod

# All old sessions immediately invalidated
```

### SSH Key Management

**Separate from ET**:
- SSH keys managed independently
- Standard SSH best practices apply

**Best Practices**:
- Use Ed25519 keys (modern, secure)
- Protect private keys (chmod 600)
- Use SSH agent (avoid storing unencrypted keys)
- Rotate keys periodically

---

## Deployment Security

### Production Deployment Checklist

**Server Hardening**:
- [ ] Run etserver-prod as non-root user
- [ ] Bind to specific interface (not 0.0.0.0)
- [ ] Configure firewall (ufw/iptables)
- [ ] Use strong passkey (32 bytes)
- [ ] Protect passkey file (chmod 600)
- [ ] Enable verbose logging for auditing
- [ ] Configure systemd service properly
- [ ] Set up log rotation

**Network Security**:
- [ ] Restrict ET port to trusted networks
- [ ] Use VPN or bastion for remote access
- [ ] Monitor connection attempts
- [ ] Configure fail2ban (future)

**SSH Security**:
- [ ] Disable password authentication
- [ ] Use public key authentication only
- [ ] Configure SSH host key verification
- [ ] Restrict SSH access (AllowUsers/AllowGroups)

**Monitoring**:
- [ ] Enable logging to syslog/journald
- [ ] Set up alerts for anomalies
- [ ] Monitor resource usage
- [ ] Track connection patterns

### Secure Configuration Examples

**Systemd Service** (secure):
```ini
[Unit]
Description=Eternal Terminal Server (Rust)
After=network.target

[Service]
Type=simple
User=etserver
Group=etserver
ExecStart=/usr/local/bin/etserver-prod --port 2022 --bind 127.0.0.1 -v
Restart=on-failure
RestartSec=5s

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/etserver

StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

**Configuration File** (secure):
```toml
# ~/.et/config.toml
[defaults]
# Use Ed25519 keys
identity_file = "/home/user/.ssh/id_ed25519"

# Don't store passkeys here (use environment or secure vault)

[[hosts]]
pattern = "*.prod.example.com"
# Production servers use bastion
jumphost = "bastion.example.com"
user = "deploy"
```

---

## Monitoring and Auditing

### Logging

**What to Log**:
- ✅ Connection attempts (success/failure)
- ✅ Client IDs
- ✅ Source IPs
- ✅ Session start/end times
- ✅ Disconnection reasons
- ✅ Protocol errors
- ❌ Passkeys (never log)
- ❌ Terminal content (privacy)

**Log Locations**:
- systemd/journald: `journalctl -u etserver-prod`
- Syslog: `/var/log/syslog`
- Custom: `--log-file /var/log/etserver/et.log`

**Example Log Entries**:
```
[INFO] Listening on 127.0.0.1:2022
[INFO] New connection from 127.0.0.1:54321
[INFO] Client registered: client-abc123
[INFO] Session established: client-abc123
[INFO] Connection closed: client-abc123 (reason: client disconnect)
```

### Metrics to Monitor

**Connection Metrics**:
- Active connections
- Connections per minute
- Failed connection attempts
- Average session duration

**Resource Metrics**:
- CPU usage
- Memory usage
- File descriptors
- Network bandwidth

**Security Metrics**:
- Authentication failures
- Encryption errors
- Abnormal disconnections
- Connection sources (IP addresses)

### Alerting

**Alert Conditions**:
- High number of failed connections (possible attack)
- Unexpected source IPs
- Resource exhaustion
- Server crashes
- Encryption errors

**Example Monitoring** (Prometheus + Grafana):
```
# Future: Export metrics in Prometheus format
# GET /metrics
et_connections_active 5
et_connections_total 1234
et_auth_failures_total 2
et_cpu_usage_percent 15.3
et_memory_bytes 45678901
```

---

## Security Best Practices

### For Administrators

1. **Use Strong Passkeys**
   ```bash
   # Generate 256-bit passkey
   openssl rand -base64 32 > /etc/et/passkey
   ```

2. **Restrict Network Access**
   ```bash
   # Only allow from trusted networks
   sudo ufw allow from 10.0.0.0/8 to any port 2022
   ```

3. **Run as Non-Root**
   ```bash
   # Create dedicated user
   sudo useradd -r -s /bin/false etserver

   # Use CAP_NET_BIND_SERVICE instead of root
   sudo setcap 'cap_net_bind_service=+ep' /usr/local/bin/etserver-prod
   ```

4. **Monitor Logs**
   ```bash
   # Watch for anomalies
   sudo journalctl -u etserver-prod -f

   # Alert on failures
   sudo journalctl -u etserver-prod -p err --since "1 hour ago"
   ```

5. **Keep Updated**
   - Subscribe to security advisories
   - Apply patches promptly
   - Test updates in staging first

### For Users

1. **Use SSH Key Authentication**
   ```bash
   # Generate key
   ssh-keygen -t ed25519

   # Add to server
   ssh-copy-id user@server
   ```

2. **Protect Private Keys**
   ```bash
   # Correct permissions
   chmod 700 ~/.ssh
   chmod 600 ~/.ssh/id_ed25519
   ```

3. **Verify Host Keys**
   ```bash
   # Check fingerprint on first connection
   ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
   ```

4. **Use Configuration Files**
   ```toml
   # ~/.et/config.toml
   [[hosts]]
   pattern = "work-*"
   identity_file = "/home/user/.ssh/work_key"
   ```

5. **Disconnect When Done**
   - Don't leave sessions open indefinitely
   - Use screen/tmux for persistence, not just ET

### For Developers

1. **Code Review**
   - All crypto code reviewed by security expert
   - No custom crypto algorithms
   - Use well-tested libraries

2. **Dependency Management**
   ```bash
   # Audit dependencies
   cargo audit

   # Keep dependencies updated
   cargo update
   ```

3. **Memory Safety**
   - Leverage Rust's memory safety
   - Use `unsafe` only when necessary
   - Audit all `unsafe` blocks

4. **Input Validation**
   - Validate all inputs
   - Sanitize data before use
   - Prevent injection attacks

5. **Testing**
   - Unit tests for crypto functions
   - Integration tests for protocol
   - Fuzz testing for parsers

---

## Vulnerability Reporting

### Reporting Security Issues

**Do NOT** open public issues for security vulnerabilities.

**Contact**:
- Email: security@example.com (if applicable)
- PGP Key: [Public key fingerprint]

**Include**:
- Description of vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if available)

### Disclosure Timeline

1. **Day 0**: Vulnerability reported
2. **Day 1**: Acknowledgment sent
3. **Day 7**: Initial assessment complete
4. **Day 30**: Fix developed and tested
5. **Day 45**: Patch released
6. **Day 90**: Public disclosure (if not sooner)

### Security Advisories

- Posted to: GitHub Security Advisories
- Format: CVE (if applicable)
- Severity rating: CVSS score

---

## Security Checklist

### Pre-Deployment

- [ ] Strong passkey generated (32 bytes)
- [ ] Passkey file permissions set (600)
- [ ] Firewall configured
- [ ] Server binds to specific interface
- [ ] SSH keys configured
- [ ] SSH password auth disabled
- [ ] systemd service configured securely
- [ ] Logging enabled

### Post-Deployment

- [ ] Monitor logs for first 48 hours
- [ ] Verify connections work
- [ ] Test from expected networks
- [ ] Verify blocked from unexpected networks
- [ ] Check resource usage
- [ ] Set up alerts
- [ ] Document configuration
- [ ] Train users

### Ongoing

- [ ] Review logs weekly
- [ ] Rotate passkeys quarterly
- [ ] Update dependencies monthly
- [ ] Audit access annually
- [ ] Test backups regularly
- [ ] Review firewall rules periodically

---

## Known Limitations

### Current Security Limitations

1. **No Per-User Passkeys**
   - All users share same passkey
   - Compromise affects all users
   - **Mitigation**: Use SSH for user authentication

2. **No Key Rotation Protocol**
   - Passkey change requires restart
   - All sessions disconnected on change
   - **Future**: Implement key rotation protocol

3. **No Rate Limiting**
   - Brute force attempts not limited
   - **Mitigation**: Use firewall rules, fail2ban (future)

4. **No Session Resume After Server Restart**
   - Server restart drops all sessions
   - **Future**: Implement session persistence

5. **Traffic Analysis Possible**
   - Packet sizes/timing visible
   - **Mitigation**: Use VPN for additional protection

---

## Compliance Considerations

### PCI DSS

If handling payment card data:
- ✅ Encryption in transit (XSalsa20-Poly1305)
- ✅ Strong authentication (SSH keys)
- ✅ Logging and monitoring
- ❌ Not designed for PCI DSS compliance specifically

### HIPAA

If handling health information:
- ✅ Encryption in transit
- ✅ Access controls (SSH authentication)
- ✅ Audit logging
- ⚠️ Consult compliance expert before use

### GDPR

- ✅ No personal data stored by ET
- ✅ Terminal content is ephemeral
- ✅ Logs can be configured to minimize PII

---

## References

### Cryptographic Standards

- **XSalsa20**: [Bernstein, D.J. "Extending the Salsa20 nonce"](https://cr.yp.to/snuffle/xsalsa-20110204.pdf)
- **Poly1305**: [Bernstein, D.J. "The Poly1305-AES message-authentication code"](https://cr.yp.to/mac/poly1305-20050329.pdf)
- **NaCl**: [Bernstein et al. "Cryptography in NaCl"](https://nacl.cr.yp.to/)

### Security Best Practices

- OWASP Top 10
- CIS Benchmarks
- NIST Cybersecurity Framework

### Related Documentation

- `DEPLOYMENT_GUIDE.md`: Production deployment
- `TROUBLESHOOTING_GUIDE.md`: Security-related troubleshooting
- `CONFIGURATION_GUIDE.md`: Secure configuration examples

---

**Security is a shared responsibility. Stay vigilant, keep updated, and report issues promptly.**

*ET Rust Security Guide v1.0*
*Last Updated: 2025-11-16*
