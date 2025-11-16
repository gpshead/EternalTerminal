# Extended End-to-End Testing Guide

## Overview

This document describes comprehensive end-to-end (E2E) testing for the Eternal Terminal Rust implementation. While Phase 7 validated protocol compatibility, Phase 8 E2E testing validates complete production workflows and real-world scenarios.

## Testing Scope

### Phase 7 vs Phase 8 Testing

**Phase 7 (Protocol Validation)**:
- ✅ TCP connection establishment
- ✅ Protocol handshake (ConnectRequest/ConnectResponse)
- ✅ Basic packet exchange
- ✅ Encryption compatibility
- ✅ Cross-implementation communication (Rust ↔ C++)

**Phase 8 (Extended E2E)**:
- Complete production flows (SSH integration)
- Error recovery and reconnection
- Session management lifecycle
- Terminal interaction (PTY, input/output, resize)
- Multi-client scenarios
- Long-running stability
- Real-world network conditions

---

## Test Categories

### 1. Full Production Flow Tests

#### Test 1.1: Rust Client → Rust Server (Full Stack)

**Components**:
- et-rs (production client)
- SSH (for remote terminal spawning)
- etterminal-rs (remote PTY process)
- etserver-prod (production server)

**Flow**:
```
et-rs → SSH connection → spawn etterminal-rs → connect to etserver-prod → interactive session
```

**Prerequisites**:
```bash
# Server setup
sudo systemctl start etserver-prod
# or
etserver-prod --port 2022 --bind 0.0.0.0 -v

# Client setup
ssh-keygen -t rsa -f ~/.ssh/id_rsa  # If needed
ssh-copy-id localhost  # For local testing
```

**Test Command**:
```bash
# Full production connection
et-rs localhost

# With verbose output
et-rs -v localhost

# With custom ports
et-rs --ssh-port 22 --et-port 2022 localhost
```

**Expected Results**:
- ✓ SSH connection established
- ✓ etterminal-rs spawned on remote server
- ✓ ET protocol connection to etserver-prod
- ✓ Interactive terminal session
- ✓ Terminal I/O working
- ✓ Clean disconnection on exit

**Validation Checklist**:
- [ ] Can type and see echoed characters
- [ ] Can run commands (ls, pwd, echo, etc.)
- [ ] Terminal colors work correctly
- [ ] Ctrl-C sends SIGINT to remote process
- [ ] Ctrl-D exits cleanly
- [ ] Terminal resize updates remote PTY
- [ ] No zombie processes left after exit

---

#### Test 1.2: Rust Client → C++ Server (Hybrid Stack)

**Components**:
- et-rs (Rust production client)
- SSH (for remote terminal spawning)
- etterminal (C++ remote PTY process)
- etserver (C++ production server)

**Flow**:
```
et-rs → SSH → spawn etterminal (C++) → connect to etserver (C++) → interactive session
```

**Prerequisites**:
```bash
# Start C++ server
/home/user/EternalTerminal/build/etserver --port 2023 --logtostdout -v 9

# Ensure C++ etterminal is in PATH
export PATH=/home/user/EternalTerminal/build:$PATH
```

**Test Command**:
```bash
# Connect to C++ server using Rust client
et-rs --et-port 2023 localhost
```

**Expected Results**:
- ✓ Rust client successfully spawns C++ etterminal
- ✓ C++ etterminal connects to C++ etserver
- ✓ Interactive session works
- ✓ Cross-implementation compatibility validated

**Note**: This tests the migration scenario where Rust clients connect to existing C++ infrastructure.

---

#### Test 1.3: C++ Client → Rust Server (Hybrid Stack)

**Components**:
- et (C++ production client)
- SSH (for remote terminal spawning)
- etterminal-rs (Rust remote PTY process)
- etserver-prod (Rust production server)

**Flow**:
```
et (C++) → SSH → spawn etterminal-rs → connect to etserver-prod → interactive session
```

**Prerequisites**:
```bash
# Start Rust server
etserver-prod --port 2022 -v

# Ensure etterminal-rs is in PATH
export PATH=/usr/local/bin:$PATH
```

**Test Command**:
```bash
# Connect to Rust server using C++ client
/home/user/EternalTerminal/build/et --port 2022 localhost
```

**Expected Results**:
- ✓ C++ client successfully spawns Rust etterminal-rs
- ✓ Rust etterminal-rs connects to Rust etserver-prod
- ✓ Interactive session works
- ✓ Reverse cross-implementation compatibility validated

**Note**: This tests the migration scenario where existing C++ clients connect to new Rust servers.

---

### 2. Error Recovery Tests

#### Test 2.1: Network Interruption Recovery

**Scenario**: Simulate temporary network failure

**Setup**:
```bash
# Start long-running command in ET session
et-rs localhost
$ ping -i 1 localhost > /tmp/ping.log &
```

**Test Steps**:
1. Establish ET session
2. Start long-running background process
3. Simulate network interruption:
   ```bash
   # In another terminal
   sudo iptables -A OUTPUT -p tcp --dport 2022 -j DROP
   sleep 10
   sudo iptables -D OUTPUT -p tcp --dport 2022 -j DROP
   ```
4. Verify session recovers

**Expected Results**:
- ✓ Client detects connection loss
- ✓ Client attempts reconnection
- ✓ Session recovers automatically
- ✓ Background process continues running
- ✓ No data loss in ping.log

**Validation**:
```bash
# Check ping log has no gaps
tail -f /tmp/ping.log
```

---

#### Test 2.2: Server Restart Recovery

**Scenario**: Server restarts while clients are connected

**Test Steps**:
1. Establish ET session with client
2. Restart server:
   ```bash
   sudo systemctl restart etserver-prod
   ```
3. Verify client behavior

**Expected Results**:
- ✓ Client detects server disconnect
- ✓ Client attempts reconnection
- ✓ New session established (or old session resumed if server supports it)
- ✓ User notified of reconnection

**Note**: Full session resume requires additional protocol support beyond current implementation.

---

#### Test 2.3: SSH Connection Failure Handling

**Scenario**: SSH connection fails during startup

**Test Steps**:
1. Stop SSH server:
   ```bash
   sudo systemctl stop sshd
   ```
2. Attempt ET connection:
   ```bash
   et-rs localhost
   ```
3. Verify error handling

**Expected Results**:
- ✓ Clear error message: "SSH connection failed"
- ✓ No hanging or timeout
- ✓ Clean exit with non-zero status
- ✓ Helpful troubleshooting hint

**Cleanup**:
```bash
sudo systemctl start sshd
```

---

#### Test 2.4: Server Not Running Handling

**Scenario**: ET server is not running

**Test Steps**:
1. Stop ET server:
   ```bash
   sudo systemctl stop etserver-prod
   ```
2. Attempt connection:
   ```bash
   et-rs localhost
   ```
3. Verify error handling

**Expected Results**:
- ✓ Clear error message: "Connection refused" or "ET server not reachable"
- ✓ No hanging
- ✓ Clean exit
- ✓ Suggestion to check server status

**Cleanup**:
```bash
sudo systemctl start etserver-prod
```

---

### 3. Session Management Tests

#### Test 3.1: Multiple Concurrent Sessions

**Scenario**: Multiple clients connecting to the same server

**Test Steps**:
1. Start server:
   ```bash
   etserver-prod --port 2022 -v
   ```
2. Open 5 concurrent sessions:
   ```bash
   # Terminal 1
   et-rs localhost

   # Terminal 2
   et-rs localhost

   # Terminal 3
   et-rs localhost

   # Terminal 4
   et-rs localhost

   # Terminal 5
   et-rs localhost
   ```
3. Interact with all sessions simultaneously

**Expected Results**:
- ✓ All 5 sessions establish successfully
- ✓ Each session is independent
- ✓ Commands in one session don't affect others
- ✓ Server handles concurrent I/O correctly
- ✓ No resource exhaustion
- ✓ Clean disconnection for each session

**Validation**:
```bash
# Check server has 5 active sessions
ps aux | grep etterminal-rs | wc -l  # Should show 5
```

---

#### Test 3.2: Session Identification

**Scenario**: Verify each session has unique ID

**Test Steps**:
1. Connect with verbose logging:
   ```bash
   et-rs -v localhost
   ```
2. Check client_id in logs
3. Connect again and verify different client_id

**Expected Results**:
- ✓ Each session gets unique client_id
- ✓ client_id is persistent during session
- ✓ Server logs show distinct client_ids
- ✓ No ID collisions

---

### 4. Terminal Interaction Tests

#### Test 4.1: Basic I/O Verification

**Test Commands**:
```bash
# After connecting with et-rs localhost

# Test 1: Echo
$ echo "Hello, ET!"
# Expected: "Hello, ET!" displayed

# Test 2: Multi-line output
$ ls -la /
# Expected: Full directory listing displayed correctly

# Test 3: Interactive input
$ read -p "Enter name: " name && echo "Hello, $name"
# Type a name and press Enter
# Expected: Prompt and response work correctly

# Test 4: stderr output
$ ls /nonexistent 2>&1
# Expected: Error message displayed

# Test 5: Background process
$ sleep 5 &
$ jobs
# Expected: Shows background job
```

**Expected Results**:
- ✓ All output displays correctly
- ✓ Input works interactively
- ✓ stderr and stdout both work
- ✓ Job control works

---

#### Test 4.2: Terminal Resize Handling

**Test Steps**:
1. Connect to ET session:
   ```bash
   et-rs localhost
   ```
2. Run command that uses full terminal:
   ```bash
   $ top
   ```
3. Resize terminal window
4. Verify display updates correctly

**Expected Results**:
- ✓ top updates to new terminal size
- ✓ No display corruption
- ✓ SIGWINCH signal handled correctly
- ✓ PTY dimensions updated

**Validation**:
```bash
# Check terminal size is correct
$ tput cols  # Should match local terminal
$ tput lines  # Should match local terminal
```

---

#### Test 4.3: Special Character Handling

**Test Commands**:
```bash
# Test special characters
$ echo "Special: !@#$%^&*(){}[]|\\:;\"'<>,.?/~\`"

# Test non-ASCII characters
$ echo "UTF-8: 你好世界 🦀 Rust 🚀"

# Test control characters
$ printf "\x1b[31mRed Text\x1b[0m\n"

# Test tab completion
$ cd /us<TAB>  # Should complete to /usr/

# Test history
$ history
$ !10  # Run command from history
```

**Expected Results**:
- ✓ Special characters display correctly
- ✓ UTF-8 characters work
- ✓ ANSI escape codes render colors
- ✓ Tab completion works
- ✓ Command history works

---

#### Test 4.4: Terminal Mode Testing

**Test Commands**:
```bash
# Test raw mode (vim)
$ vim /tmp/test.txt
# Type some text, save, quit
# Expected: vim works normally

# Test canonical mode
$ cat > /tmp/file.txt
Type some text
Press Ctrl-D
# Expected: Works correctly

# Test password input (no echo)
$ sudo ls  # Enter password
# Expected: Password not echoed
```

**Expected Results**:
- ✓ Raw mode programs (vim, nano) work correctly
- ✓ Canonical mode input works
- ✓ Password input (no echo) works
- ✓ Terminal modes switch correctly

---

### 5. Long-Running Stability Tests

#### Test 5.1: 24-Hour Stability Test

**Scenario**: Verify session remains stable over extended period

**Test Steps**:
1. Start ET session
2. Run monitoring script:
   ```bash
   #!/bin/bash
   # save as monitor.sh
   while true; do
       date >> /tmp/et-stability.log
       uptime >> /tmp/et-stability.log
       sleep 60
   done
   ```
3. Run in background:
   ```bash
   et-rs localhost
   $ ./monitor.sh &
   $ exit  # Keep session alive by not fully disconnecting
   ```
4. Check after 24 hours

**Expected Results**:
- ✓ Session remains connected
- ✓ No memory leaks
- ✓ Log file shows continuous timestamps
- ✓ No connection drops
- ✓ Server resources stable

**Validation**:
```bash
# Check log continuity
grep -c "$(date +%Y-%m-%d)" /tmp/et-stability.log  # Should be ~1440 (24*60)

# Check for gaps
awk '{print $4}' /tmp/et-stability.log | uniq -c | grep -v "^  *2 "  # Should find no gaps
```

---

#### Test 5.2: High Throughput Test

**Scenario**: Transfer large amount of data through session

**Test Steps**:
1. Connect to ET session
2. Transfer large file:
   ```bash
   $ dd if=/dev/zero bs=1M count=1024 | pv | md5sum
   ```
3. Monitor throughput

**Expected Results**:
- ✓ High throughput maintained
- ✓ No data corruption
- ✓ No connection instability
- ✓ Memory usage remains reasonable

---

### 6. Configuration Integration Tests

#### Test 6.1: Config File Loading

**Setup**:
```bash
# Create test config
mkdir -p ~/.et
cat > ~/.et/config.toml << 'EOF'
[defaults]
verbose = true
et_port = 2022

[[hosts]]
pattern = "localhost"
user = "testuser"
EOF
```

**Test Steps**:
1. Connect without CLI args:
   ```bash
   et-rs localhost
   ```
2. Verify config is applied

**Expected Results**:
- ✓ Verbose mode enabled (shows debug output)
- ✓ Port 2022 used
- ✓ Username from config applied
- ✓ Config loading logged

---

#### Test 6.2: CLI Override

**Test Steps**:
1. With config from Test 6.1 (verbose = true)
2. Connect with CLI override:
   ```bash
   et-rs --et-port 2023 localhost
   ```

**Expected Results**:
- ✓ Port 2023 used (CLI overrides config)
- ✓ Verbose still enabled (from config)
- ✓ Priority system working correctly

---

#### Test 6.3: Pattern Matching

**Setup**:
```bash
cat > ~/.et/config.toml << 'EOF'
[[hosts]]
pattern = "*.local"
et_port = 3000

[[hosts]]
pattern = "prod-*"
et_port = 2022
identity_file = "~/.ssh/prod_key"
EOF
```

**Test Steps**:
1. Test wildcard matching:
   ```bash
   et-rs server.local  # Should use port 3000
   et-rs prod-web-01   # Should use port 2022 and prod_key
   ```

**Expected Results**:
- ✓ Pattern matching works
- ✓ Correct settings applied per pattern
- ✓ Wildcard expansion works

---

### 7. Security Tests

#### Test 7.1: Passkey Validation

**Scenario**: Mismatched passkeys should be rejected

**Test Steps**:
1. Start server with passkey:
   ```bash
   etserver-prod --port 2022 -k "server-secret"
   ```
2. Start etterminal with different passkey:
   ```bash
   etterminal-rs --serverport 2022 --passkey "wrong-secret"
   ```

**Expected Results**:
- ✓ Connection rejected
- ✓ Decryption failure detected
- ✓ Clear error message
- ✓ No crash or hang

---

#### Test 7.2: Encryption Verification

**Scenario**: Verify traffic is encrypted

**Test Steps**:
1. Start packet capture:
   ```bash
   sudo tcpdump -i lo -w /tmp/et-traffic.pcap port 2022
   ```
2. Establish ET session and run commands
3. Analyze capture:
   ```bash
   strings /tmp/et-traffic.pcap | grep "secret-data"
   ```

**Expected Results**:
- ✓ No plaintext visible in packet capture
- ✓ All data encrypted
- ✓ XSalsa20-Poly1305 in use

---

### 8. Cross-Version Compatibility Tests

#### Test 8.1: Rust v6.2.11 ↔ C++ v6.2.11

**Test Matrix**:

| Client | Server | Expected Result |
|--------|--------|-----------------|
| Rust et-rs | Rust etserver-prod | ✅ Full compatibility |
| Rust et-rs | C++ etserver | ✅ Full compatibility |
| C++ et | Rust etserver-prod | ✅ Full compatibility |
| C++ et | C++ etserver | ✅ Baseline (known working) |

**Test Steps**:
For each combination:
1. Start server
2. Connect with client
3. Run interactive session
4. Verify all features work

**Expected Results**:
- ✓ All combinations work
- ✓ Protocol version 6 compatible
- ✓ No feature degradation

---

### 9. Performance Baseline Tests

**Note**: Detailed performance benchmarking moved to Phase 9. These are basic sanity checks.

#### Test 9.1: Connection Latency

**Test**:
```bash
time et-rs localhost "echo test"
```

**Expected Results**:
- ✓ Connection establishes in < 2 seconds
- ✓ No excessive delays
- ✓ Reasonable performance

---

#### Test 9.2: Interactive Responsiveness

**Test**:
```bash
et-rs localhost
$ time ls /usr/bin
```

**Expected Results**:
- ✓ No noticeable lag
- ✓ Output streams smoothly
- ✓ Interactive feel similar to local terminal

---

## Test Execution Guide

### Automated Test Suite

For tests that can be automated, use this test runner:

```bash
#!/bin/bash
# save as run-e2e-tests.sh

set -e

echo "=== Eternal Terminal E2E Test Suite ==="
echo ""

# Test 1: Basic connectivity
echo "Test 1: Basic Connectivity"
et-rs localhost "echo 'Connection test passed'" || { echo "FAIL: Basic connectivity"; exit 1; }
echo "✓ PASS"
echo ""

# Test 2: Command execution
echo "Test 2: Command Execution"
et-rs localhost "ls /tmp && pwd && echo \$SHELL" || { echo "FAIL: Command execution"; exit 1; }
echo "✓ PASS"
echo ""

# Test 3: Multiple sequential connections
echo "Test 3: Multiple Sequential Connections"
for i in {1..5}; do
    et-rs localhost "echo 'Connection $i'" || { echo "FAIL: Connection $i"; exit 1; }
done
echo "✓ PASS"
echo ""

# Test 4: Config file loading
echo "Test 4: Config File Loading"
if [ -f ~/.et/config.toml ]; then
    et-rs -v localhost "echo 'Config loaded'" 2>&1 | grep -q "config" && echo "✓ PASS" || echo "⚠ Config may not be loaded"
else
    echo "⊘ SKIP: No config file"
fi
echo ""

echo "=== E2E Test Suite Complete ==="
```

### Manual Test Checklist

Use this checklist for manual testing:

- [ ] Full production flow (Rust → Rust)
- [ ] Hybrid flow (Rust client → C++ server)
- [ ] Hybrid flow (C++ client → Rust server)
- [ ] Network interruption recovery
- [ ] Server restart recovery
- [ ] SSH failure handling
- [ ] ET server not running handling
- [ ] Multiple concurrent sessions (5 clients)
- [ ] Session identification (unique IDs)
- [ ] Basic I/O (echo, ls, read)
- [ ] Terminal resize (run top and resize window)
- [ ] Special characters (UTF-8, ANSI colors)
- [ ] Raw mode programs (vim)
- [ ] 24-hour stability test
- [ ] High throughput (large file transfer)
- [ ] Config file loading
- [ ] CLI override priority
- [ ] Pattern matching in config
- [ ] Passkey validation (reject wrong passkey)
- [ ] Encryption verification (tcpdump)
- [ ] Cross-version compatibility (all 4 combinations)
- [ ] Connection latency (< 2s)
- [ ] Interactive responsiveness (no lag)

---

## Test Results Documentation

### Result Format

For each test, document:

```markdown
#### Test X.Y: Test Name

**Date**: YYYY-MM-DD
**Tester**: Name
**Environment**: OS, ET version

**Result**: ✅ PASS / ❌ FAIL / ⚠ PARTIAL / ⊘ SKIP

**Details**:
- What worked
- What didn't work
- Unexpected behavior
- Performance observations

**Logs**: (attach relevant logs)

**Recommendations**: (if applicable)
```

---

## Known Limitations

### Current Implementation Gaps

1. **Port Forwarding**: Not yet implemented (Phase 6 added CLI parsing only)
2. **Multi-hop Jumphosts**: Not yet implemented (Phase 6 added CLI parsing only)
3. **Session Resume After Server Restart**: Requires additional protocol support
4. **HTM Mode**: Headless terminal multiplexer mode not implemented

These limitations do not affect core functionality but should be noted when testing.

---

## Troubleshooting Test Failures

### Test Fails: Connection Refused

**Possible Causes**:
1. Server not running
2. Wrong port
3. Firewall blocking

**Debug Steps**:
```bash
# Check server is running
ps aux | grep etserver-prod

# Check port is listening
sudo netstat -tlnp | grep 2022

# Test connectivity
telnet localhost 2022
```

### Test Fails: SSH Connection Failed

**Possible Causes**:
1. SSH server not running
2. SSH keys not configured
3. SSH permissions wrong

**Debug Steps**:
```bash
# Test SSH directly
ssh localhost

# Check SSH service
sudo systemctl status sshd

# Check key permissions
ls -la ~/.ssh/id_rsa  # Should be 600
```

### Test Fails: Encryption Error

**Possible Causes**:
1. Passkey mismatch
2. Protocol version mismatch

**Debug Steps**:
```bash
# Check server logs
sudo journalctl -u etserver-prod -n 50

# Run with verbose logging
et-rs -v localhost
```

---

## Appendix: Test Environment Setup

### Minimal Test Environment

```bash
# Install dependencies
sudo apt-get update
sudo apt-get install -y openssh-server build-essential

# Build Rust ET
cd /home/user/EternalTerminal/et-rust
cargo build --release

# Install binaries
sudo cp target/release/etserver-prod /usr/local/bin/
sudo cp target/release/etterminal-rs /usr/local/bin/
sudo cp target/release/et-rs /usr/local/bin/

# Setup SSH
ssh-keygen -t rsa -N "" -f ~/.ssh/id_rsa
cat ~/.ssh/id_rsa.pub >> ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys

# Start server
etserver-prod --port 2022 -v &

# Verify
ps aux | grep etserver-prod
```

### Full Test Environment

Includes both Rust and C++ for cross-compatibility testing:

```bash
# Build C++ ET
cd /home/user/EternalTerminal
mkdir -p build
cd build
cmake ../
make -j$(nproc)

# Install C++ binaries (optional, for testing)
sudo cp et /usr/local/bin/et-cpp
sudo cp etserver /usr/local/bin/etserver-cpp
sudo cp etterminal /usr/local/bin/etterminal-cpp

# Now have both Rust and C++ for full compatibility matrix
```

---

## Phase 8 E2E Testing Summary

Extended E2E testing validates:

1. ✅ **Complete Production Flows**: Full stack testing beyond protocol validation
2. ✅ **Error Recovery**: Network failures, server restarts, connection issues
3. ✅ **Session Management**: Multiple concurrent sessions, unique IDs
4. ✅ **Terminal Interaction**: I/O, resize, special characters, terminal modes
5. ✅ **Long-Running Stability**: 24-hour tests, high throughput
6. ✅ **Configuration Integration**: Config loading, CLI overrides, pattern matching
7. ✅ **Security**: Passkey validation, encryption verification
8. ✅ **Cross-Version Compatibility**: Rust ↔ Rust, Rust ↔ C++ in both directions

This comprehensive test suite ensures the Rust implementation is production-ready and fully compatible with existing C++ deployments.

---

*Last Updated: 2025-11-16*
*ET Rust Implementation v6.2.11*
*Phase 8: Production Hardening & Documentation*
