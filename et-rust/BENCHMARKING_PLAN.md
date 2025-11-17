# Phase 9: Performance Benchmarking Plan

## Overview

This document outlines the comprehensive performance benchmarking methodology for the Eternal Terminal Rust implementation. Benchmarks compare Rust ET against C++ ET across multiple dimensions.

**Version**: 6.2.11
**Date**: 2025-11-16
**Environment**: Sandbox (results should be validated in production)

---

## Table of Contents

1. [Benchmarking Goals](#benchmarking-goals)
2. [Test Environment](#test-environment)
3. [Benchmark Categories](#benchmark-categories)
4. [Methodology](#methodology)
5. [Metrics to Collect](#metrics-to-collect)
6. [Test Scenarios](#test-scenarios)
7. [Analysis Plan](#analysis-plan)
8. [Success Criteria](#success-criteria)

---

## Benchmarking Goals

### Primary Goals

1. **Validate Performance Parity**: Confirm Rust implementation performs comparably to C++
2. **Identify Bottlenecks**: Discover any performance issues in Rust implementation
3. **Establish Baselines**: Create reference measurements for future optimization
4. **Guide Production Deployment**: Provide data for capacity planning

### Secondary Goals

1. **Build Time Validation**: Confirm faster build times with Rust
2. **Binary Size Validation**: Confirm smaller binaries with Rust
3. **Resource Efficiency**: Compare CPU and memory usage
4. **Scalability**: Test concurrent connection handling

---

## Test Environment

### Sandbox Environment

**Platform**: Linux 4.4.0
**Limitations**:
- No systemd
- Limited SSH infrastructure
- Potential performance characteristics different from production
- Resource constraints unknown

**Caveats**:
- Absolute numbers may not represent production performance
- Relative comparisons (Rust vs C++) still valid
- Methodology is production-ready

### Hardware Information

```bash
# CPU
cat /proc/cpuinfo | grep "model name" | head -1
lscpu | grep -E "CPU\(s\)|Model name|Thread|Core"

# Memory
free -h

# Disk
df -h /

# Network
ip addr
```

### Software Versions

- **Rust**: `rustc --version`
- **C++ Compiler**: `gcc --version` / `g++ --version`
- **CMake**: `cmake --version`
- **ET C++**: Version from build
- **ET Rust**: 6.2.11

---

## Benchmark Categories

### 1. Build Performance

**What**: Measure compilation speed

**Why**: Faster builds = faster development iteration

**Metrics**:
- Full clean build time
- Incremental rebuild time (1 file changed)
- Clean time (time to remove build artifacts)

**Tests**:
- C++ full build (`cmake .. && make clean && time make -j$(nproc)`)
- Rust full build (`cargo clean && time cargo build --release`)
- C++ incremental rebuild
- Rust incremental rebuild

---

### 2. Binary Sizes

**What**: Measure compiled binary sizes

**Why**: Smaller binaries = faster deployment, less disk space

**Metrics**:
- Client binary size
- Server binary size
- Terminal binary size
- Total deployment size

**Tests**:
- Measure all binary sizes (release builds)
- Compare stripped vs non-stripped
- Compare debug vs release

---

### 3. Connection Latency

**What**: Measure time to establish connection

**Why**: Low latency = better user experience

**Metrics**:
- Time to establish ET connection
- Time for full handshake
- Time to first terminal output
- Connection overhead vs plain SSH

**Tests**:
- 100 sequential connections
- Measure min/max/avg/p50/p95/p99
- Compare Rust vs C++

---

### 4. Throughput

**What**: Measure data transfer rates

**Why**: High throughput = better for data-intensive tasks

**Metrics**:
- Bytes per second (upload)
- Bytes per second (download)
- Symmetric throughput
- Efficiency vs TCP raw throughput

**Tests**:
- Transfer large file through ET session
- Measure with dd/pv
- Compare encryption overhead

---

### 5. CPU Usage

**What**: Measure CPU consumption

**Why**: Lower CPU = more efficient, can handle more connections

**Metrics**:
- Idle CPU usage (server with connections)
- Active CPU usage (during data transfer)
- CPU per connection
- CPU efficiency (work/CPU)

**Tests**:
- Monitor with `top` / `pidstat`
- Measure during idle and active
- Compare Rust vs C++

---

### 6. Memory Usage

**What**: Measure memory consumption

**Why**: Lower memory = can handle more connections, better resource efficiency

**Metrics**:
- Base memory (server startup)
- Memory per connection
- Memory under load
- Memory leaks (over time)

**Tests**:
- Monitor with `ps` / `/proc/PID/status`
- Measure RSS, VmSize
- Long-running test for leaks

---

### 7. Concurrent Connections

**What**: Measure scalability with multiple clients

**Why**: Server must handle many simultaneous connections

**Metrics**:
- Max concurrent connections
- Performance degradation curve
- Resource usage scaling
- Connection accept rate

**Tests**:
- 1, 5, 10, 25, 50 concurrent connections
- Measure latency/throughput at each level
- Find breaking point

---

### 8. Protocol Overhead

**What**: Measure encryption/protocol overhead

**Why**: Lower overhead = more efficient

**Metrics**:
- Encryption time
- Decryption time
- Packet serialization time
- Total protocol overhead vs raw TCP

**Tests**:
- Microbenchmarks for crypto operations
- Compare with raw TCP socket

---

## Methodology

### General Principles

1. **Isolation**: Run one test at a time
2. **Repetition**: Run each test multiple times (minimum 3)
3. **Warmup**: Discard first run (cache warming)
4. **Consistency**: Same test conditions for all runs
5. **Documentation**: Record all parameters and conditions

### Test Execution Order

1. Build performance (clean environment)
2. Binary sizes (after builds complete)
3. Server startup and idle (establish baseline)
4. Connection latency (quick tests)
5. Throughput (medium duration)
6. Concurrent connections (increasing load)
7. Long-running tests (stability)

### Data Collection

```bash
# Template for each test
TEST_NAME="connection_latency"
IMPLEMENTATION="rust"  # or "cpp"
ITERATION=1

# Record start conditions
date > "${TEST_NAME}_${IMPLEMENTATION}_${ITERATION}.log"
free -h >> "${TEST_NAME}_${IMPLEMENTATION}_${ITERATION}.log"

# Run test
time ./run_test.sh >> "${TEST_NAME}_${IMPLEMENTATION}_${ITERATION}.log" 2>&1

# Record end conditions
free -h >> "${TEST_NAME}_${IMPLEMENTATION}_${ITERATION}.log"
```

### Statistical Analysis

- **Central Tendency**: Mean, Median
- **Spread**: Standard Deviation, Min/Max
- **Percentiles**: p50, p95, p99
- **Comparison**: Rust vs C++ percentage difference

---

## Metrics to Collect

### Build Metrics

| Metric | Unit | Collection Method |
|--------|------|-------------------|
| Full build time | seconds | `time make` / `time cargo build` |
| Incremental build time | seconds | Touch 1 file, rebuild |
| Clean time | seconds | `time make clean` / `time cargo clean` |
| Parallel speedup | ratio | Compare -j1 vs -j$(nproc) |

### Binary Metrics

| Metric | Unit | Collection Method |
|--------|------|-------------------|
| Client size | MB | `ls -lh et` / `ls -lh et-rs` |
| Server size | MB | `ls -lh etserver` / `ls -lh etserver-prod` |
| Terminal size | MB | `ls -lh etterminal` / `ls -lh etterminal-rs` |
| Stripped size | MB | After `strip` |

### Runtime Metrics

| Metric | Unit | Collection Method |
|--------|------|-------------------|
| Connection time | ms | `time et localhost` |
| First byte latency | ms | Time to first output |
| Throughput | MB/s | `dd if=/dev/zero bs=1M count=100 | pv` |
| CPU % | percent | `top -b -n1 | grep etserver` |
| Memory (RSS) | MB | `ps aux | grep etserver` |
| Connections/sec | conn/s | Sequential connection loop |

### Scalability Metrics

| Metric | Unit | Collection Method |
|--------|------|-------------------|
| Connections active | count | `netstat | grep ESTABLISHED | wc -l` |
| CPU per connection | percent | CPU% / connection count |
| Memory per connection | MB | Memory increase / new connections |
| Latency degradation | ms | p95 latency vs connection count |

---

## Test Scenarios

### Scenario 1: Quick Connection Test

**Purpose**: Measure connection establishment speed

**Procedure**:
```bash
# Start server
etserver-prod --port 2022 &

# Test 100 connections
for i in {1..100}; do
    time et-rs localhost "echo test" >> /dev/null 2>&1
done
```

**Metrics**: Min, max, avg, p95 connection time

---

### Scenario 2: Bulk Data Transfer

**Purpose**: Measure throughput

**Procedure**:
```bash
# Connect to server
et-rs localhost

# Transfer 100MB
dd if=/dev/zero bs=1M count=100 | pv | md5sum
```

**Metrics**: MB/s, total time, CPU usage during transfer

---

### Scenario 3: Concurrent Clients

**Purpose**: Test scalability

**Procedure**:
```bash
# Start N clients in parallel
for i in {1..10}; do
    et-rs localhost "sleep 60" &
done

# Monitor server
pidstat -p $(pgrep etserver-prod) 1 60
```

**Metrics**: CPU%, memory, latency per connection count

---

### Scenario 4: Long-Running Stability

**Purpose**: Test memory leaks and stability

**Procedure**:
```bash
# Connect and run for 1 hour
et-rs localhost "while true; do echo test; sleep 1; done"

# Monitor memory every 10 seconds
while true; do
    ps aux | grep etserver-prod | grep -v grep >> memory_log.txt
    sleep 10
done
```

**Metrics**: Memory growth over time, CPU stability

---

### Scenario 5: Interactive Responsiveness

**Purpose**: Measure user experience

**Procedure**:
```bash
# Connect to server
et-rs localhost

# Type commands and measure lag
echo "test" # Measure time to see output
ls /usr/bin # Measure output streaming
```

**Metrics**: Keystroke latency, output latency

---

## Analysis Plan

### Data Processing

```bash
# Calculate statistics
cat results.txt | awk '{sum+=$1; sumsq+=$1*$1} END {
    print "Mean:", sum/NR;
    print "StdDev:", sqrt(sumsq/NR - (sum/NR)^2)
}'

# Calculate percentiles
sort -n results.txt | awk '{a[NR]=$1} END {
    print "p50:", a[int(NR*0.5)];
    print "p95:", a[int(NR*0.95)];
    print "p99:", a[int(NR*0.99)]
}'
```

### Comparison Methodology

```bash
# Rust vs C++ comparison
RUST_AVG=123.4
CPP_AVG=130.2

DIFF=$((RUST_AVG - CPP_AVG))
PERCENT=$(echo "scale=2; ($DIFF / $CPP_AVG) * 100" | bc)

echo "Rust is ${PERCENT}% faster" # if negative
# or
echo "Rust is ${PERCENT}% slower" # if positive
```

### Visualization

- Bar charts for build times
- Line graphs for throughput
- Scatter plots for latency distribution
- Tables for resource usage

---

## Success Criteria

### Must Have (P0)

- ✅ Rust connection latency within 10% of C++
- ✅ Rust throughput within 10% of C++
- ✅ Rust CPU usage within 20% of C++
- ✅ Rust memory usage within 20% of C++
- ✅ No memory leaks (constant memory over time)

### Should Have (P1)

- ✅ Rust build time faster than C++ (already validated)
- ✅ Rust binary size smaller than C++ (already validated)
- ✅ Rust handles 50+ concurrent connections
- ✅ Rust p95 latency < 100ms (localhost)

### Nice to Have (P2)

- 🎯 Rust outperforms C++ in any metric
- 🎯 Rust scales better with concurrent connections
- 🎯 Rust has lower jitter (more consistent performance)

---

## Benchmark Execution Checklist

### Pre-Benchmark

- [ ] Clean environment (no other processes)
- [ ] Build both C++ and Rust in release mode
- [ ] Verify binaries are correct versions
- [ ] Create results directory
- [ ] Document environment specs
- [ ] Start system monitoring

### During Benchmark

- [ ] Run tests in consistent order
- [ ] Allow cooldown between tests
- [ ] Monitor system resources
- [ ] Log all commands and output
- [ ] Verify tests complete successfully

### Post-Benchmark

- [ ] Collect all logs
- [ ] Process raw data
- [ ] Calculate statistics
- [ ] Generate comparison tables
- [ ] Create visualizations
- [ ] Write analysis report
- [ ] Archive raw data

---

## Benchmark Automation

### Directory Structure

```
benchmarks/
├── scripts/
│   ├── run-all-benchmarks.sh
│   ├── build-benchmarks.sh
│   ├── latency-benchmark.sh
│   ├── throughput-benchmark.sh
│   ├── concurrency-benchmark.sh
│   └── resource-benchmark.sh
├── results/
│   ├── rust/
│   ├── cpp/
│   └── comparison/
├── data/
│   └── raw/
└── reports/
    └── PERFORMANCE_REPORT.md
```

### Main Benchmark Script

```bash
#!/bin/bash
# run-all-benchmarks.sh

./scripts/build-benchmarks.sh
./scripts/latency-benchmark.sh
./scripts/throughput-benchmark.sh
./scripts/concurrency-benchmark.sh
./scripts/resource-benchmark.sh

# Generate report
./scripts/generate-report.sh
```

---

## Environment Documentation Template

```markdown
## Test Environment

**Date**: YYYY-MM-DD HH:MM:SS UTC
**Platform**: Linux 4.4.0
**CPU**: [from /proc/cpuinfo]
**Cores**: [count]
**Memory**: [total]
**Disk**: [type and space]

**Software**:
- Rust: [version]
- GCC: [version]
- CMake: [version]
- ET C++: [version]
- ET Rust: 6.2.11

**Build Flags**:
- C++: [flags]
- Rust: --release

**Network**: Localhost (lo interface)

**Notes**:
- Sandbox environment
- Limited systemd
- Results for methodology validation
```

---

## Known Limitations

### Sandbox Environment

- ⚠️ Performance may not represent production hardware
- ⚠️ Limited SSH infrastructure (affects full E2E tests)
- ⚠️ Unknown resource constraints
- ⚠️ May have unusual scheduling characteristics

### Test Limitations

- 🔸 Full E2E tests require SSH infrastructure
- 🔸 Network tests limited to localhost (no real network latency)
- 🔸 Cannot test real-world network conditions
- 🔸 Cannot test actual deployment scenarios

### Mitigation

- ✅ Focus on relative comparisons (Rust vs C++)
- ✅ Document methodology for production testing
- ✅ Establish baseline for future comparisons
- ✅ Test what's testable, document what's not

---

## Future Enhancements

### Phase 9+

- Flame graphs for CPU profiling
- Memory allocation profiling (valgrind, massif)
- Network simulation (latency, packet loss)
- Load testing with realistic workloads
- Long-term stability testing (7+ days)
- Comparison under production conditions

---

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Linux Performance Tools](https://www.brendangregg.com/linuxperf.html)
- [Benchmarking Best Practices](https://pyperf.readthedocs.io/en/latest/run_benchmark.html)

---

*Phase 9: Performance Benchmarking Plan*
*ET Rust Implementation v6.2.11*
*Last Updated: 2025-11-16*
