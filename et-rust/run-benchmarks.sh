#!/bin/bash
# Comprehensive Performance Benchmarking Suite for ET Rust vs C++
# Phase 9: Performance Benchmarking

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create results directory
RESULTS_DIR="benchmark-results-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RESULTS_DIR"/{rust,cpp,comparison,raw}

echo "=============================================="
echo "  ET Performance Benchmark Suite"
echo "  Version: 6.2.11"
echo "  Date: $(date)"
echo "  Results: $RESULTS_DIR"
echo "=============================================="
echo ""

# Log environment
log_environment() {
    echo "=== Test Environment ===" > "$RESULTS_DIR/environment.txt"
    echo "Date: $(date)" >> "$RESULTS_DIR/environment.txt"
    echo "" >> "$RESULTS_DIR/environment.txt"

    echo "Platform:" >> "$RESULTS_DIR/environment.txt"
    uname -a >> "$RESULTS_DIR/environment.txt"
    echo "" >> "$RESULTS_DIR/environment.txt"

    echo "CPU:" >> "$RESULTS_DIR/environment.txt"
    cat /proc/cpuinfo | grep "model name" | head -1 >> "$RESULTS_DIR/environment.txt"
    echo "Cores: $(nproc)" >> "$RESULTS_DIR/environment.txt"
    echo "" >> "$RESULTS_DIR/environment.txt"

    echo "Memory:" >> "$RESULTS_DIR/environment.txt"
    free -h >> "$RESULTS_DIR/environment.txt"
    echo "" >> "$RESULTS_DIR/environment.txt"

    echo "Rust Version:" >> "$RESULTS_DIR/environment.txt"
    rustc --version >> "$RESULTS_DIR/environment.txt"
    cargo --version >> "$RESULTS_DIR/environment.txt"
    echo "" >> "$RESULTS_DIR/environment.txt"

    echo "C++ Compiler:" >> "$RESULTS_DIR/environment.txt"
    g++ --version | head -1 >> "$RESULTS_DIR/environment.txt"
    cmake --version | head -1 >> "$RESULTS_DIR/environment.txt"
    echo "" >> "$RESULTS_DIR/environment.txt"
}

log_environment
cat "$RESULTS_DIR/environment.txt"

# ============================================
# Benchmark 1: Build Performance
# ============================================

benchmark_build_performance() {
    echo ""
    echo -e "${BLUE}=== Benchmark 1: Build Performance ===${NC}"
    echo ""

    # Rust build time
    echo -e "${GREEN}Testing Rust build performance...${NC}"
    cd /home/user/EternalTerminal/et-rust

    echo "Full clean build..."
    cargo clean > /dev/null 2>&1
    RUST_BUILD_TIME=$( { time cargo build --release > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' )
    echo "Rust full build: $RUST_BUILD_TIME"
    echo "$RUST_BUILD_TIME" > "$RESULTS_DIR/rust/build_time_full.txt"

    echo "Incremental rebuild (no changes)..."
    RUST_REBUILD_TIME=$( { time cargo build --release > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' )
    echo "Rust incremental: $RUST_REBUILD_TIME"
    echo "$RUST_REBUILD_TIME" > "$RESULTS_DIR/rust/build_time_incremental.txt"

    # C++ build time
    echo ""
    echo -e "${GREEN}Testing C++ build performance...${NC}"
    cd /home/user/EternalTerminal/build

    echo "Full clean build..."
    make clean > /dev/null 2>&1 || true
    CPP_BUILD_TIME=$( { time make -j$(nproc) > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' )
    echo "C++ full build: $CPP_BUILD_TIME"
    echo "$CPP_BUILD_TIME" > "$RESULTS_DIR/cpp/build_time_full.txt"

    echo "Incremental rebuild (no changes)..."
    CPP_REBUILD_TIME=$( { time make -j$(nproc) > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' )
    echo "C++ incremental: $CPP_REBUILD_TIME"
    echo "$CPP_REBUILD_TIME" > "$RESULTS_DIR/cpp/build_time_incremental.txt"

    cd /home/user/EternalTerminal
}

# ============================================
# Benchmark 2: Binary Sizes
# ============================================

benchmark_binary_sizes() {
    echo ""
    echo -e "${BLUE}=== Benchmark 2: Binary Sizes ===${NC}"
    echo ""

    # Rust binaries
    echo -e "${GREEN}Rust binaries:${NC}"
    RUST_CLIENT_SIZE=$(ls -lh /home/user/EternalTerminal/et-rust/target/release/et-rs 2>/dev/null | awk '{print $5}' || echo "N/A")
    RUST_SERVER_SIZE=$(ls -lh /home/user/EternalTerminal/et-rust/target/release/etserver-prod 2>/dev/null | awk '{print $5}' || echo "N/A")
    RUST_TERMINAL_SIZE=$(ls -lh /home/user/EternalTerminal/et-rust/target/release/etterminal-rs 2>/dev/null | awk '{print $5}' || echo "N/A")

    echo "  et-rs:          $RUST_CLIENT_SIZE"
    echo "  etserver-prod:  $RUST_SERVER_SIZE"
    echo "  etterminal-rs:  $RUST_TERMINAL_SIZE"

    echo "$RUST_CLIENT_SIZE" > "$RESULTS_DIR/rust/binary_size_client.txt"
    echo "$RUST_SERVER_SIZE" > "$RESULTS_DIR/rust/binary_size_server.txt"
    echo "$RUST_TERMINAL_SIZE" > "$RESULTS_DIR/rust/binary_size_terminal.txt"

    # C++ binaries
    echo ""
    echo -e "${GREEN}C++ binaries:${NC}"
    CPP_CLIENT_SIZE=$(ls -lh /home/user/EternalTerminal/build/et 2>/dev/null | awk '{print $5}' || echo "N/A")
    CPP_SERVER_SIZE=$(ls -lh /home/user/EternalTerminal/build/etserver 2>/dev/null | awk '{print $5}' || echo "N/A")
    CPP_TERMINAL_SIZE=$(ls -lh /home/user/EternalTerminal/build/etterminal 2>/dev/null | awk '{print $5}' || echo "N/A")

    echo "  et:             $CPP_CLIENT_SIZE"
    echo "  etserver:       $CPP_SERVER_SIZE"
    echo "  etterminal:     $CPP_TERMINAL_SIZE"

    echo "$CPP_CLIENT_SIZE" > "$RESULTS_DIR/cpp/binary_size_client.txt"
    echo "$CPP_SERVER_SIZE" > "$RESULTS_DIR/cpp/binary_size_server.txt"
    echo "$CPP_TERMINAL_SIZE" > "$RESULTS_DIR/cpp/binary_size_terminal.txt"
}

# ============================================
# Benchmark 3: Memory Usage (Baseline)
# ============================================

benchmark_memory_baseline() {
    echo ""
    echo -e "${BLUE}=== Benchmark 3: Memory Usage (Baseline) ===${NC}"
    echo ""

    # Rust server memory
    echo -e "${GREEN}Testing Rust server memory...${NC}"
    /home/user/EternalTerminal/et-rust/target/release/etserver-prod --port 3022 --bind 127.0.0.1 > /dev/null 2>&1 &
    RUST_PID=$!
    sleep 2

    RUST_MEM=$(ps -p $RUST_PID -o rss= 2>/dev/null || echo "0")
    RUST_MEM_MB=$(echo "scale=2; $RUST_MEM / 1024" | bc)
    echo "Rust server (idle): ${RUST_MEM_MB} MB"
    echo "$RUST_MEM_MB" > "$RESULTS_DIR/rust/memory_baseline.txt"

    kill $RUST_PID 2>/dev/null || true
    sleep 1

    # C++ server memory
    echo ""
    echo -e "${GREEN}Testing C++ server memory...${NC}"
    /home/user/EternalTerminal/build/etserver --port 3023 > /dev/null 2>&1 &
    CPP_PID=$!
    sleep 2

    CPP_MEM=$(ps -p $CPP_PID -o rss= 2>/dev/null || echo "0")
    CPP_MEM_MB=$(echo "scale=2; $CPP_MEM / 1024" | bc)
    echo "C++ server (idle): ${CPP_MEM_MB} MB"
    echo "$CPP_MEM_MB" > "$RESULTS_DIR/cpp/memory_baseline.txt"

    kill $CPP_PID 2>/dev/null || true
    sleep 1
}

# ============================================
# Benchmark 4: Protocol Performance
# ============================================

benchmark_protocol_performance() {
    echo ""
    echo -e "${BLUE}=== Benchmark 4: Protocol Performance ===${NC}"
    echo ""

    # Test Rust protocol client/server
    echo -e "${GREEN}Testing Rust protocol performance...${NC}"

    # Start Rust server
    /home/user/EternalTerminal/et-rust/target/release/etserver-rs --port 4022 > /dev/null 2>&1 &
    RUST_SERVER_PID=$!
    sleep 1

    # Run protocol test (10 iterations)
    echo "Running 10 protocol iterations..."
    RUST_TIMES=()
    for i in {1..10}; do
        START=$(date +%s%N)
        timeout 5 /home/user/EternalTerminal/et-rust/target/debug/etclient-rs --host 127.0.0.1 --port 4022 -n 3 > /dev/null 2>&1 || true
        END=$(date +%s%N)
        ELAPSED=$(echo "scale=3; ($END - $START) / 1000000" | bc)
        RUST_TIMES+=($ELAPSED)
        echo "  Iteration $i: ${ELAPSED}ms"
    done

    # Calculate average
    RUST_AVG=$(printf '%s\n' "${RUST_TIMES[@]}" | awk '{sum+=$1} END {print sum/NR}')
    echo "Rust protocol average: ${RUST_AVG}ms"
    echo "$RUST_AVG" > "$RESULTS_DIR/rust/protocol_latency.txt"

    kill $RUST_SERVER_PID 2>/dev/null || true
    sleep 1

    echo ""
    echo -e "${YELLOW}Note: C++ protocol test would require similar infrastructure${NC}"
}

# ============================================
# Benchmark 5: Startup Time
# ============================================

benchmark_startup_time() {
    echo ""
    echo -e "${BLUE}=== Benchmark 5: Server Startup Time ===${NC}"
    echo ""

    # Rust server startup
    echo -e "${GREEN}Testing Rust server startup...${NC}"
    RUST_STARTUP_TIMES=()
    for i in {1..5}; do
        START=$(date +%s%N)
        /home/user/EternalTerminal/et-rust/target/release/etserver-prod --port 5022 --bind 127.0.0.1 > /dev/null 2>&1 &
        PID=$!
        while ! netstat -tln 2>/dev/null | grep -q ":5022.*LISTEN"; do
            sleep 0.01
        done
        END=$(date +%s%N)
        ELAPSED=$(echo "scale=3; ($END - $START) / 1000000" | bc)
        RUST_STARTUP_TIMES+=($ELAPSED)
        echo "  Iteration $i: ${ELAPSED}ms"
        kill $PID 2>/dev/null || true
        sleep 0.5
    done

    RUST_STARTUP_AVG=$(printf '%s\n' "${RUST_STARTUP_TIMES[@]}" | awk '{sum+=$1} END {print sum/NR}')
    echo "Rust server startup average: ${RUST_STARTUP_AVG}ms"
    echo "$RUST_STARTUP_AVG" > "$RESULTS_DIR/rust/startup_time.txt"

    # C++ server startup
    echo ""
    echo -e "${GREEN}Testing C++ server startup...${NC}"
    CPP_STARTUP_TIMES=()
    for i in {1..5}; do
        START=$(date +%s%N)
        /home/user/EternalTerminal/build/etserver --port 5023 > /dev/null 2>&1 &
        PID=$!
        while ! netstat -tln 2>/dev/null | grep -q ":5023.*LISTEN"; do
            sleep 0.01
        done
        END=$(date +%s%N)
        ELAPSED=$(echo "scale=3; ($END - $START) / 1000000" | bc)
        CPP_STARTUP_TIMES+=($ELAPSED)
        echo "  Iteration $i: ${ELAPSED}ms"
        kill $PID 2>/dev/null || true
        sleep 0.5
    done

    CPP_STARTUP_AVG=$(printf '%s\n' "${CPP_STARTUP_TIMES[@]}" | awk '{sum+=$1} END {print sum/NR}')
    echo "C++ server startup average: ${CPP_STARTUP_AVG}ms"
    echo "$CPP_STARTUP_AVG" > "$RESULTS_DIR/cpp/startup_time.txt"
}

# ============================================
# Benchmark 6: CPU Usage Under Load
# ============================================

benchmark_cpu_usage() {
    echo ""
    echo -e "${BLUE}=== Benchmark 6: CPU Usage ===${NC}"
    echo ""

    echo -e "${GREEN}Testing Rust server CPU usage...${NC}"
    /home/user/EternalTerminal/et-rust/target/release/etserver-rs --port 6022 > /dev/null 2>&1 &
    RUST_PID=$!
    sleep 2

    # Baseline CPU (idle)
    RUST_CPU_IDLE=$(ps -p $RUST_PID -o %cpu= 2>/dev/null || echo "0")
    echo "Rust CPU (idle): ${RUST_CPU_IDLE}%"
    echo "$RUST_CPU_IDLE" > "$RESULTS_DIR/rust/cpu_idle.txt"

    kill $RUST_PID 2>/dev/null || true
    sleep 1

    echo ""
    echo -e "${GREEN}Testing C++ server CPU usage...${NC}"
    /home/user/EternalTerminal/build/etserver --port 6023 > /dev/null 2>&1 &
    CPP_PID=$!
    sleep 2

    CPP_CPU_IDLE=$(ps -p $CPP_PID -o %cpu= 2>/dev/null || echo "0")
    echo "C++ CPU (idle): ${CPP_CPU_IDLE}%"
    echo "$CPP_CPU_IDLE" > "$RESULTS_DIR/cpp/cpu_idle.txt"

    kill $CPP_PID 2>/dev/null || true
}

# ============================================
# Run All Benchmarks
# ============================================

echo -e "${YELLOW}Starting benchmark suite...${NC}"
echo ""

benchmark_build_performance
benchmark_binary_sizes
benchmark_memory_baseline
benchmark_protocol_performance
benchmark_startup_time
benchmark_cpu_usage

# ============================================
# Generate Summary
# ============================================

echo ""
echo "=============================================="
echo -e "${GREEN}Benchmark Suite Complete!${NC}"
echo "=============================================="
echo ""
echo "Results saved to: $RESULTS_DIR"
echo ""
echo "Summary:"
echo "--------"
echo ""

# Build Performance
echo "Build Performance:"
RUST_BUILD=$(cat "$RESULTS_DIR/rust/build_time_full.txt")
CPP_BUILD=$(cat "$RESULTS_DIR/cpp/build_time_full.txt")
echo "  Rust full build:  $RUST_BUILD"
echo "  C++ full build:   $CPP_BUILD"
echo ""

# Binary Sizes
echo "Binary Sizes:"
echo "  Rust et-rs:       $(cat "$RESULTS_DIR/rust/binary_size_client.txt")"
echo "  C++ et:           $(cat "$RESULTS_DIR/cpp/binary_size_client.txt")"
echo "  Rust etserver:    $(cat "$RESULTS_DIR/rust/binary_size_server.txt")"
echo "  C++ etserver:     $(cat "$RESULTS_DIR/cpp/binary_size_server.txt")"
echo ""

# Memory
echo "Memory Usage (Idle):"
echo "  Rust server:      $(cat "$RESULTS_DIR/rust/memory_baseline.txt") MB"
echo "  C++ server:       $(cat "$RESULTS_DIR/cpp/memory_baseline.txt") MB"
echo ""

# Startup Time
echo "Server Startup Time:"
echo "  Rust:             $(cat "$RESULTS_DIR/rust/startup_time.txt") ms"
echo "  C++ :             $(cat "$RESULTS_DIR/cpp/startup_time.txt") ms"
echo ""

echo "See $RESULTS_DIR/ for detailed results"
echo ""

# Create summary file
cat > "$RESULTS_DIR/SUMMARY.txt" << EOF
ET Rust vs C++ Performance Benchmark Summary
============================================
Date: $(date)

Build Performance:
  Rust full build:       $RUST_BUILD
  C++ full build:        $CPP_BUILD
  Rust incremental:      $(cat "$RESULTS_DIR/rust/build_time_incremental.txt")
  C++ incremental:       $(cat "$RESULTS_DIR/cpp/build_time_incremental.txt")

Binary Sizes:
  Rust et-rs:            $(cat "$RESULTS_DIR/rust/binary_size_client.txt")
  C++ et:                $(cat "$RESULTS_DIR/cpp/binary_size_client.txt")
  Rust etserver-prod:    $(cat "$RESULTS_DIR/rust/binary_size_server.txt")
  C++ etserver:          $(cat "$RESULTS_DIR/cpp/binary_size_server.txt")
  Rust etterminal-rs:    $(cat "$RESULTS_DIR/rust/binary_size_terminal.txt")
  C++ etterminal:        $(cat "$RESULTS_DIR/cpp/binary_size_terminal.txt")

Memory Usage (Idle):
  Rust server:           $(cat "$RESULTS_DIR/rust/memory_baseline.txt") MB
  C++ server:            $(cat "$RESULTS_DIR/cpp/memory_baseline.txt") MB

Server Startup Time:
  Rust:                  $(cat "$RESULTS_DIR/rust/startup_time.txt") ms
  C++:                   $(cat "$RESULTS_DIR/cpp/startup_time.txt") ms

CPU Usage (Idle):
  Rust:                  $(cat "$RESULTS_DIR/rust/cpu_idle.txt") %
  C++:                   $(cat "$RESULTS_DIR/cpp/cpu_idle.txt") %

Protocol Performance:
  Rust avg latency:      $(cat "$RESULTS_DIR/rust/protocol_latency.txt") ms

Environment: See environment.txt for details
EOF

echo "Summary saved to: $RESULTS_DIR/SUMMARY.txt"
echo ""
echo "✅ Benchmarking complete!"
