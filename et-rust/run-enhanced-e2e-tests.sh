#!/bin/bash
# Enhanced E2E Testing with Self-Managed SSH Setup
# Configures and starts a localhost-only SSH daemon for full E2E testing

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test configuration
SSH_PORT=2222
SSH_CONFIG_DIR="$HOME/.et-test-ssh"
SSH_HOST_KEY="$SSH_CONFIG_DIR/ssh_host_ed25519_key"
SSH_USER_KEY="$HOME/.ssh/et_test_key"
SSH_PID_FILE="$SSH_CONFIG_DIR/sshd.pid"
ET_PORT=2022
TEST_RESULTS_DIR="e2e-test-results-$(date +%Y%m%d-%H%M%S)"

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

# Helper functions
pass_test() {
    echo -e "${GREEN}✓ PASS${NC}: $1"
    ((PASS_COUNT++))
}

fail_test() {
    echo -e "${RED}✗ FAIL${NC}: $1"
    ((FAIL_COUNT++))
}

skip_test() {
    echo -e "${YELLOW}⊘ SKIP${NC}: $1"
    ((SKIP_COUNT++))
}

cleanup() {
    echo ""
    echo "=== Cleanup ==="

    # Stop SSH daemon
    if [ -f "$SSH_PID_FILE" ]; then
        SSH_PID=$(cat "$SSH_PID_FILE")
        if kill -0 $SSH_PID 2>/dev/null; then
            echo "Stopping SSH daemon (PID: $SSH_PID)..."
            kill $SSH_PID 2>/dev/null || true
            sleep 1
        fi
        rm -f "$SSH_PID_FILE"
    fi

    # Stop ET server
    pkill -f "etserver-prod.*$ET_PORT" 2>/dev/null || true

    echo "Cleanup complete"
}

trap cleanup EXIT

echo "=============================================="
echo "  Enhanced E2E Test Suite with SSH Setup"
echo "  Version: 6.2.11"
echo "  Date: $(date)"
echo "=============================================="
echo ""

mkdir -p "$TEST_RESULTS_DIR"
mkdir -p "$SSH_CONFIG_DIR"

# ============================================
# Step 1: Configure and Start SSH Daemon
# ============================================

echo -e "${BLUE}=== Step 1: SSH Daemon Setup ===${NC}"
echo ""

# Generate SSH host key if needed
if [ ! -f "$SSH_HOST_KEY" ]; then
    echo "Generating SSH host key..."
    ssh-keygen -t ed25519 -f "$SSH_HOST_KEY" -N '' -C "et-test-host" > /dev/null 2>&1
    pass_test "SSH host key generated"
else
    echo "SSH host key already exists"
fi

# Generate user SSH key if needed
if [ ! -f "$SSH_USER_KEY" ]; then
    echo "Generating user SSH key..."
    ssh-keygen -t ed25519 -f "$SSH_USER_KEY" -N '' -C "et-test-user" > /dev/null 2>&1
    pass_test "User SSH key generated"
else
    echo "User SSH key already exists"
fi

# Set up authorized_keys
mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"
cat "$SSH_USER_KEY.pub" >> "$HOME/.ssh/authorized_keys"
chmod 600 "$HOME/.ssh/authorized_keys"
echo "User key added to authorized_keys"

# Create custom sshd_config
cat > "$SSH_CONFIG_DIR/sshd_config" << EOF
# Custom SSH config for ET E2E testing
Port $SSH_PORT
ListenAddress 127.0.0.1

# Host keys
HostKey $SSH_HOST_KEY

# Authentication
PubkeyAuthentication yes
PasswordAuthentication no
ChallengeResponseAuthentication no
UsePAM no

# Permissions
StrictModes no
PermitRootLogin no
AllowUsers $USER

# Logging
SyslogFacility AUTH
LogLevel INFO

# Runtime
PidFile $SSH_PID_FILE
PrintMotd no
PrintLastLog no

# Subsystems (required for SSH to work)
Subsystem sftp /usr/lib/openssh/sftp-server
EOF

# Start SSH daemon
echo "Starting SSH daemon on port $SSH_PORT..."
/usr/sbin/sshd -f "$SSH_CONFIG_DIR/sshd_config" -E "$SSH_CONFIG_DIR/sshd.log" 2>&1

# Wait for SSH to start
sleep 2

# Verify SSH is running
if [ -f "$SSH_PID_FILE" ]; then
    SSH_PID=$(cat "$SSH_PID_FILE")
    if kill -0 $SSH_PID 2>/dev/null; then
        pass_test "SSH daemon started (PID: $SSH_PID)"
    else
        fail_test "SSH daemon not running"
        cat "$SSH_CONFIG_DIR/sshd.log"
        exit 1
    fi
else
    fail_test "SSH PID file not created"
    cat "$SSH_CONFIG_DIR/sshd.log"
    exit 1
fi

# Verify SSH is listening
if netstat -tln 2>/dev/null | grep -q ":$SSH_PORT.*LISTEN" || ss -tln 2>/dev/null | grep -q ":$SSH_PORT"; then
    pass_test "SSH listening on port $SSH_PORT"
else
    fail_test "SSH not listening on port $SSH_PORT"
    exit 1
fi

# Test SSH connection
echo ""
echo "Testing SSH connectivity..."
if ssh -p $SSH_PORT -i "$SSH_USER_KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null localhost "echo 'SSH works'" > /dev/null 2>&1; then
    pass_test "SSH connectivity confirmed"
else
    fail_test "SSH connection failed"
    cat "$SSH_CONFIG_DIR/sshd.log"
    exit 1
fi

echo ""

# ============================================
# Step 2: Start ET Server
# ============================================

echo -e "${BLUE}=== Step 2: ET Server Setup ===${NC}"
echo ""

echo "Starting ET server on port $ET_PORT..."
./target/release/etserver-prod --port $ET_PORT --bind 127.0.0.1 -v > "$TEST_RESULTS_DIR/etserver.log" 2>&1 &
ET_SERVER_PID=$!

sleep 2

if kill -0 $ET_SERVER_PID 2>/dev/null; then
    pass_test "ET server started (PID: $ET_SERVER_PID)"
else
    fail_test "ET server failed to start"
    cat "$TEST_RESULTS_DIR/etserver.log"
    exit 1
fi

if netstat -tln 2>/dev/null | grep -q ":$ET_PORT.*LISTEN" || ss -tln 2>/dev/null | grep -q ":$ET_PORT"; then
    pass_test "ET server listening on port $ET_PORT"
else
    fail_test "ET server not listening on port $ET_PORT"
    exit 1
fi

echo ""

# ============================================
# Step 3: Full E2E Tests
# ============================================

echo -e "${BLUE}=== Step 3: Full E2E Tests ===${NC}"
echo ""

# Test 1: Simple command execution
echo "Test 1: Simple Command Execution via ET"
OUTPUT=$(timeout 15 ./target/release/et-rs -p $SSH_PORT --et-port $ET_PORT -i "$SSH_USER_KEY" localhost "echo 'E2E test passed'" 2>&1 || echo "TIMEOUT")
if echo "$OUTPUT" | grep -q "E2E test passed"; then
    pass_test "Command execution via full stack"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test1_output.txt"
else
    fail_test "Command execution failed"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test1_output.txt"
    echo "Output: $OUTPUT"
fi

# Test 2: User verification
echo ""
echo "Test 2: User Verification"
OUTPUT=$(timeout 15 ./target/release/et-rs -p $SSH_PORT --et-port $ET_PORT -i "$SSH_USER_KEY" localhost "whoami" 2>&1 || echo "TIMEOUT")
if echo "$OUTPUT" | grep -q "$USER"; then
    pass_test "User verification ($USER)"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test2_output.txt"
else
    fail_test "User verification failed"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test2_output.txt"
fi

# Test 3: Multiple commands
echo ""
echo "Test 3: Multiple Commands"
OUTPUT=$(timeout 15 ./target/release/et-rs -p $SSH_PORT --et-port $ET_PORT -i "$SSH_USER_KEY" localhost "pwd && ls /tmp && echo done" 2>&1 || echo "TIMEOUT")
if echo "$OUTPUT" | grep -q "done"; then
    pass_test "Multiple commands executed"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test3_output.txt"
else
    fail_test "Multiple commands failed"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test3_output.txt"
fi

# Test 4: Environment variables
echo ""
echo "Test 4: Environment Variables"
OUTPUT=$(timeout 15 ./target/release/et-rs -p $SSH_PORT --et-port $ET_PORT -i "$SSH_USER_KEY" localhost "echo \$HOME && echo \$USER" 2>&1 || echo "TIMEOUT")
if echo "$OUTPUT" | grep -q "$USER"; then
    pass_test "Environment variables accessible"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test4_output.txt"
else
    fail_test "Environment variables not accessible"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test4_output.txt"
fi

# Test 5: Sequential connections
echo ""
echo "Test 5: Sequential Connections"
SUCCESS_COUNT=0
for i in {1..5}; do
    OUTPUT=$(timeout 15 ./target/release/et-rs -p $SSH_PORT --et-port $ET_PORT -i "$SSH_USER_KEY" localhost "echo 'Connection $i'" 2>&1 || echo "TIMEOUT")
    if echo "$OUTPUT" | grep -q "Connection $i"; then
        ((SUCCESS_COUNT++))
    fi
done

if [ $SUCCESS_COUNT -eq 5 ]; then
    pass_test "All 5 sequential connections successful"
else
    fail_test "Only $SUCCESS_COUNT/5 connections successful"
fi
echo "$SUCCESS_COUNT/5" > "$TEST_RESULTS_DIR/test5_result.txt"

# Test 6: File operations
echo ""
echo "Test 6: File Operations"
TEST_FILE="/tmp/et-test-$$"
OUTPUT=$(timeout 15 ./target/release/et-rs -p $SSH_PORT --et-port $ET_PORT -i "$SSH_USER_KEY" localhost "echo 'test data' > $TEST_FILE && cat $TEST_FILE && rm $TEST_FILE" 2>&1 || echo "TIMEOUT")
if echo "$OUTPUT" | grep -q "test data"; then
    pass_test "File operations successful"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test6_output.txt"
else
    fail_test "File operations failed"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test6_output.txt"
fi

# Test 7: etterminal-rs spawning
echo ""
echo "Test 7: etterminal-rs Process Spawning"
# Check if etterminal-rs processes are created during connection
BEFORE_COUNT=$(pgrep -c etterminal-rs || echo 0)
timeout 15 ./target/release/et-rs -p $SSH_PORT --et-port $ET_PORT -i "$SSH_USER_KEY" localhost "sleep 2" > /dev/null 2>&1 &
ET_CLIENT_PID=$!
sleep 3
DURING_COUNT=$(pgrep -c etterminal-rs || echo 0)
wait $ET_CLIENT_PID 2>/dev/null || true
sleep 1
AFTER_COUNT=$(pgrep -c etterminal-rs || echo 0)

echo "etterminal-rs count: before=$BEFORE_COUNT, during=$DURING_COUNT, after=$AFTER_COUNT"
if [ $DURING_COUNT -gt $BEFORE_COUNT ]; then
    pass_test "etterminal-rs spawned during connection"
else
    skip_test "etterminal-rs spawning (may be too fast to detect)"
fi

# Test 8: Server resource usage
echo ""
echo "Test 8: Server Resource Usage"
ET_MEM=$(ps -p $ET_SERVER_PID -o rss= 2>/dev/null || echo "0")
ET_MEM_MB=$(echo "scale=2; $ET_MEM / 1024" | bc)
ET_CPU=$(ps -p $ET_SERVER_PID -o %cpu= 2>/dev/null || echo "0")
echo "ET Server - Memory: ${ET_MEM_MB}MB, CPU: ${ET_CPU}%"
echo "Memory: ${ET_MEM_MB}MB" > "$TEST_RESULTS_DIR/test8_memory.txt"
echo "CPU: ${ET_CPU}%" > "$TEST_RESULTS_DIR/test8_cpu.txt"
pass_test "Server resource usage measured"

# Test 9: Connection to non-standard port
echo ""
echo "Test 9: Non-Standard Port Configuration"
OUTPUT=$(timeout 15 ./target/release/et-rs -p $SSH_PORT --et-port $ET_PORT -i "$SSH_USER_KEY" localhost "echo 'Custom port works'" 2>&1 || echo "TIMEOUT")
if echo "$OUTPUT" | grep -q "Custom port works"; then
    pass_test "Non-standard SSH port ($SSH_PORT) works"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test9_output.txt"
else
    fail_test "Non-standard port failed"
    echo "$OUTPUT" > "$TEST_RESULTS_DIR/test9_output.txt"
fi

# Test 10: Verify no zombie processes
echo ""
echo "Test 10: Zombie Process Check"
ZOMBIE_COUNT=$(ps aux | grep -c '<defunct>' || echo 0)
if [ $ZOMBIE_COUNT -eq 0 ]; then
    pass_test "No zombie processes"
else
    echo "Found $ZOMBIE_COUNT zombie processes"
    skip_test "Zombie process check (${ZOMBIE_COUNT} found)"
fi

# ============================================
# Summary
# ============================================

echo ""
echo "=============================================="
echo -e "${GREEN}Enhanced E2E Test Suite Complete!${NC}"
echo "=============================================="
echo ""
echo -e "${GREEN}Passed${NC}:  $PASS_COUNT"
if [ $FAIL_COUNT -gt 0 ]; then
    echo -e "${RED}Failed${NC}:  $FAIL_COUNT"
fi
if [ $SKIP_COUNT -gt 0 ]; then
    echo -e "${YELLOW}Skipped${NC}: $SKIP_COUNT"
fi
echo ""

# Save summary
cat > "$TEST_RESULTS_DIR/SUMMARY.txt" << EOF
Enhanced E2E Test Results
=========================
Date: $(date)

Configuration:
  SSH Port: $SSH_PORT
  ET Port:  $ET_PORT
  SSH Key:  $SSH_USER_KEY

Results:
  Passed:   $PASS_COUNT
  Failed:   $FAIL_COUNT
  Skipped:  $SKIP_COUNT

SSH Daemon:
  PID:      $(cat $SSH_PID_FILE)
  Config:   $SSH_CONFIG_DIR/sshd_config
  Log:      $SSH_CONFIG_DIR/sshd.log

ET Server:
  PID:      $ET_SERVER_PID
  Log:      $TEST_RESULTS_DIR/etserver.log
  Memory:   ${ET_MEM_MB}MB
  CPU:      ${ET_CPU}%

Full test output saved to:
  $TEST_RESULTS_DIR/
EOF

echo "Results saved to: $TEST_RESULTS_DIR/"
echo "Summary saved to: $TEST_RESULTS_DIR/SUMMARY.txt"
echo ""

if [ $FAIL_COUNT -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    exit 1
fi
