#!/bin/bash
# Manual benchmark collection

RESULTS="benchmark-results-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RESULTS"/{rust,cpp,comparison}

echo "==============================================="
echo "ET Rust vs C++ Performance Benchmarks"
echo "Date: $(date)"
echo "==============================================="
echo ""

# Environment
echo "=== Environment ===" | tee "$RESULTS/environment.txt"
echo "Platform: $(uname -a)" | tee -a "$RESULTS/environment.txt"
echo "CPU: $(cat /proc/cpuinfo | grep 'model name' | head -1 | cut -d: -f2 | xargs)" | tee -a "$RESULTS/environment.txt"
echo "Cores: $(nproc)" | tee -a "$RESULTS/environment.txt"
echo "Memory: $(free -h | grep Mem | awk '{print $2}')" | tee -a "$RESULTS/environment.txt"
echo "Rust: $(rustc --version)" | tee -a "$RESULTS/environment.txt"
echo "GCC: $(gcc --version | head -1)" | tee -a "$RESULTS/environment.txt"
echo ""

# Binary Sizes
echo "=== Binary Sizes ===" | tee "$RESULTS/binary_sizes.txt"
echo "Rust Binaries:" | tee -a "$RESULTS/binary_sizes.txt"
ls -lh target/release/{et-rs,etserver-prod,etterminal-rs} 2>/dev/null | awk '{printf "  %-20s %s\n", $9, $5}' | tee -a "$RESULTS/binary_sizes.txt"
echo "" | tee -a "$RESULTS/binary_sizes.txt"
echo "C++ Binaries:" | tee -a "$RESULTS/binary_sizes.txt"
ls -lh /home/user/EternalTerminal/build/{et,etserver,etterminal} 2>/dev/null | awk '{printf "  %-20s %s\n", $9, $5}' | tee -a "$RESULTS/binary_sizes.txt"
echo ""

# Build Time (Rust incremental)
echo "=== Build Performance ===" | tee "$RESULTS/build_perf.txt"
echo "Rust incremental rebuild:" | tee -a "$RESULTS/build_perf.txt"
{ time cargo build --release 2>&1 > /dev/null; } 2>&1 | grep real | tee -a "$RESULTS/build_perf.txt"
echo ""

# Server Memory (Rust)
echo "=== Memory Usage ===" | tee "$RESULTS/memory.txt"
target/release/etserver-prod --port 7022 --bind 127.0.0.1 > /dev/null 2>&1 &
RUST_PID=$!
sleep 2
RUST_MEM=$(ps -p $RUST_PID -o rss= 2>/dev/null | awk '{print $1/1024}')
echo "Rust server (idle): ${RUST_MEM} MB" | tee -a "$RESULTS/memory.txt"
kill $RUST_PID 2>/dev/null
sleep 1

# Server Memory (C++)
/home/user/EternalTerminal/build/etserver --port 7023 > /dev/null 2>&1 &
CPP_PID=$!
sleep 2
CPP_MEM=$(ps -p $CPP_PID -o rss= 2>/dev/null | awk '{print $1/1024}')
echo "C++ server (idle): ${CPP_MEM} MB" | tee -a "$RESULTS/memory.txt"
kill $CPP_PID 2>/dev/null
echo ""

# Startup time
echo "=== Startup Time ===" | tee "$RESULTS/startup.txt"
echo "Measuring Rust server startup (5 iterations)..."
RUST_TIMES=()
for i in {1..5}; do
    START=$(date +%s%N)
    target/release/etserver-prod --port 8022 --bind 127.0.0.1 > /dev/null 2>&1 &
    PID=$!
    while ! netstat -tln 2>/dev/null | grep -q ':8022.*LISTEN'; do sleep 0.01; done
    END=$(date +%s%N)
    ELAPSED=$((($END - $START) / 1000000))
    RUST_TIMES+=($ELAPSED)
    echo "  Run $i: ${ELAPSED}ms"
    kill $PID 2>/dev/null
    sleep 0.5
done
RUST_AVG=$(printf '%s\n' "${RUST_TIMES[@]}" | awk '{sum+=$1} END {print sum/NR}')
echo "Rust startup average: ${RUST_AVG}ms" | tee -a "$RESULTS/startup.txt"

echo ""
echo "Measuring C++ server startup (5 iterations)..."
CPP_TIMES=()
for i in {1..5}; do
    START=$(date +%s%N)
    /home/user/EternalTerminal/build/etserver --port 8023 > /dev/null 2>&1 &
    PID=$!
    while ! netstat -tln 2>/dev/null | grep -q ':8023.*LISTEN'; do sleep 0.01; done
    END=$(date +%s%N)
    ELAPSED=$((($END - $START) / 1000000))
    CPP_TIMES+=($ELAPSED)
    echo "  Run $i: ${ELAPSED}ms"
    kill $PID 2>/dev/null
    sleep 0.5
done
CPP_AVG=$(printf '%s\n' "${CPP_TIMES[@]}" | awk '{sum+=$1} END {print sum/NR}')
echo "C++ startup average: ${CPP_AVG}ms" | tee -a "$RESULTS/startup.txt"
echo ""

# Summary
echo "==============================================="
echo "Benchmark Complete!"
echo "Results saved to: $RESULTS/"
echo "==============================================="
