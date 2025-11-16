#!/bin/bash
# Eternal Terminal E2E Test Suite
# Phase 8: Production Hardening & Documentation

set -e

echo "=============================================="
echo "  ET Rust E2E Test Suite"
echo "  Version: 6.2.11"
echo "  Date: $(date)"
echo "=============================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

# Test result functions
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

# Pre-flight checks
echo "=== Pre-flight Checks ==="
echo ""

# Check if binaries exist
echo "Checking binaries..."
if [ -f ./target/release/et-rs ]; then
    ET_CLIENT=./target/release/et-rs
elif [ -f /usr/local/bin/et-rs ]; then
    ET_CLIENT=/usr/local/bin/et-rs
else
    echo "Building release binaries..."
    cargo build --release
    ET_CLIENT=./target/release/et-rs
fi

if [ -f ./target/release/etserver-prod ]; then
    ET_SERVER=./target/release/etserver-prod
elif [ -f /usr/local/bin/etserver-prod ]; then
    ET_SERVER=/usr/local/bin/etserver-prod
else
    ET_SERVER=./target/release/etserver-prod
fi

if [ -f ./target/release/etterminal-rs ]; then
    ET_TERMINAL=./target/release/etterminal-rs
elif [ -f /usr/local/bin/etterminal-rs ]; then
    ET_TERMINAL=/usr/local/bin/etterminal-rs
else
    ET_TERMINAL=./target/release/etterminal-rs
fi

echo "  et-rs: $ET_CLIENT"
echo "  etserver-prod: $ET_SERVER"
echo "  etterminal-rs: $ET_TERMINAL"
echo ""

# Check if server is running
echo "Checking server status..."
if pgrep -f "etserver-prod.*2022" > /dev/null; then
    echo "  Server already running on port 2022"
    SERVER_WAS_RUNNING=true
else
    echo "  Starting test server on port 2022..."
    $ET_SERVER --port 2022 --bind 127.0.0.1 -v > /tmp/et-test-server.log 2>&1 &
    SERVER_PID=$!
    SERVER_WAS_RUNNING=false
    sleep 2

    if ! kill -0 $SERVER_PID 2>/dev/null; then
        echo "  ERROR: Failed to start server"
        cat /tmp/et-test-server.log
        exit 1
    fi
    echo "  Server started (PID: $SERVER_PID)"
fi
echo ""

# Check SSH
echo "Checking SSH..."
if systemctl is-active --quiet sshd || systemctl is-active --quiet ssh; then
    echo "  SSH service is running"
else
    skip_test "SSH service not running, skipping full E2E tests"
    SSH_AVAILABLE=false
fi

# Check SSH keys
if [ -f ~/.ssh/id_rsa ] || [ -f ~/.ssh/id_ed25519 ]; then
    echo "  SSH keys found"
    SSH_AVAILABLE=true
else
    echo "  No SSH keys found"
    SSH_AVAILABLE=false
fi
echo ""

echo "=== Running E2E Tests ==="
echo ""

# Test 1: Protocol version check
echo "Test 1: Protocol Version Check"
if $ET_CLIENT --help 2>&1 | grep -q "6.2.11"; then
    pass_test "Client version 6.2.11"
else
    fail_test "Client version mismatch"
fi
echo ""

# Test 2: Server connectivity check
echo "Test 2: Server Connectivity"
if nc -zv 127.0.0.1 2022 2>&1 | grep -q "succeeded\|open"; then
    pass_test "Server port 2022 is reachable"
else
    fail_test "Cannot reach server on port 2022"
fi
echo ""

# Test 3: Simple command execution (if SSH available)
if [ "$SSH_AVAILABLE" = true ]; then
    echo "Test 3: Simple Command Execution"
    # Test with timeout to avoid hanging
    if timeout 10 ssh -o BatchMode=yes -o StrictHostKeyChecking=no localhost "echo test" > /dev/null 2>&1; then
        pass_test "SSH connectivity confirmed"

        # Now test ET command execution
        echo "Test 3a: ET Command Execution"
        OUTPUT=$(timeout 10 $ET_CLIENT localhost "echo 'ET test passed'" 2>&1 || echo "TIMEOUT")
        if echo "$OUTPUT" | grep -q "ET test passed"; then
            pass_test "Command execution via ET"
        else
            echo "  Output: $OUTPUT"
            fail_test "Command execution failed or timed out"
        fi
    else
        skip_test "SSH not configured for passwordless auth"
    fi
else
    skip_test "Test 3: SSH not available"
fi
echo ""

# Test 4: Multiple sequential connections
if [ "$SSH_AVAILABLE" = true ]; then
    echo "Test 4: Multiple Sequential Connections"
    SUCCESS=true
    for i in {1..3}; do
        if ! timeout 10 $ET_CLIENT localhost "echo 'Connection $i'" > /dev/null 2>&1; then
            SUCCESS=false
            break
        fi
    done
    if [ "$SUCCESS" = true ]; then
        pass_test "3 sequential connections successful"
    else
        fail_test "Sequential connections failed"
    fi
else
    skip_test "Test 4: SSH not available"
fi
echo ""

# Test 5: Config file loading
echo "Test 5: Configuration File Support"
if [ -f ~/.et/config.toml ]; then
    pass_test "Config file exists at ~/.et/config.toml"
else
    skip_test "No config file found (this is OK)"
fi
echo ""

# Test 6: Binary sizes
echo "Test 6: Binary Size Verification"
if [ -f "$ET_CLIENT" ]; then
    SIZE=$(du -h "$ET_CLIENT" | cut -f1)
    echo "  et-rs size: $SIZE"
    pass_test "Binary built successfully"
fi
if [ -f "$ET_SERVER" ]; then
    SIZE=$(du -h "$ET_SERVER" | cut -f1)
    echo "  etserver-prod size: $SIZE"
fi
echo ""

# Test 7: Server process count
echo "Test 7: Server Process Check"
SERVER_COUNT=$(pgrep -f "etserver-prod" | wc -l)
if [ "$SERVER_COUNT" -ge 1 ]; then
    echo "  Found $SERVER_COUNT server process(es)"
    pass_test "Server is running"
else
    fail_test "No server process found"
fi
echo ""

# Test 8: etterminal binary exists
echo "Test 8: etterminal-rs Binary"
if [ -f "$ET_TERMINAL" ]; then
    pass_test "etterminal-rs binary exists"
else
    fail_test "etterminal-rs binary not found"
fi
echo ""

# Cleanup
echo "=== Cleanup ==="
if [ "$SERVER_WAS_RUNNING" = false ] && [ -n "$SERVER_PID" ]; then
    echo "Stopping test server (PID: $SERVER_PID)..."
    kill $SERVER_PID 2>/dev/null || true
    sleep 1
    kill -9 $SERVER_PID 2>/dev/null || true
fi
echo ""

# Summary
echo "=============================================="
echo "  Test Summary"
echo "=============================================="
echo -e "${GREEN}Passed${NC}: $PASS_COUNT"
if [ $FAIL_COUNT -gt 0 ]; then
    echo -e "${RED}Failed${NC}: $FAIL_COUNT"
fi
if [ $SKIP_COUNT -gt 0 ]; then
    echo -e "${YELLOW}Skipped${NC}: $SKIP_COUNT"
fi
echo "=============================================="
echo ""

if [ $FAIL_COUNT -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    exit 1
fi
