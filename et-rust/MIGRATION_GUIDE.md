# C++ to Rust Migration Guide

## Overview

This guide helps organizations migrate from the C++ implementation of Eternal Terminal to the Rust implementation. The migration can be gradual, low-risk, and performed with zero downtime.

**Target Audience**: System administrators, DevOps engineers, IT operations
**Prerequisites**: Familiarity with ET concepts, basic understanding of deployment procedures

---

## Table of Contents

1. [Why Migrate?](#why-migrate)
2. [Compatibility](#compatibility)
3. [Migration Strategies](#migration-strategies)
4. [Pre-Migration Checklist](#pre-migration-checklist)
5. [Migration Paths](#migration-paths)
6. [Step-by-Step Migration](#step-by-step-migration)
7. [Testing and Validation](#testing-and-validation)
8. [Rollback Procedures](#rollback-procedures)
9. [Common Migration Scenarios](#common-migration-scenarios)
10. [Troubleshooting](#troubleshooting)
11. [Post-Migration](#post-migration)

---

## Why Migrate?

### Benefits of Rust Implementation

**Performance**:
- 🚀 Faster build times: 10-15x faster incremental builds
- 📉 Smaller binaries: 12-23% smaller than C++ versions
- ⚡ Similar runtime performance (async Tokio runtime)

**Development**:
- 🔧 Faster iteration: 25s full rebuild vs 3-5 minutes
- 🧪 Better testing: Cargo test infrastructure
- 📦 Easier dependency management: Cargo vs CMake

**Safety**:
- 🛡️ Memory safety: No buffer overflows, use-after-free
- 🔒 Thread safety: Compiler-enforced concurrency safety
- ✅ Fewer bugs: Type system catches errors at compile time

**Maintenance**:
- 📚 Modern codebase: Cleaner, more maintainable code
- 🔄 Active development: Ongoing improvements
- 🌐 Growing ecosystem: Rust libraries and tools

### When to Migrate

**Good Reasons**:
- ✅ Modernizing infrastructure
- ✅ Improving security posture
- ✅ Reducing maintenance burden
- ✅ Adopting Rust across organization
- ✅ Need faster build/deploy cycles

**Not Good Reasons**:
- ❌ C++ version working fine with no issues
- ❌ No resources for testing/validation
- ❌ No business case for change

### When NOT to Migrate

- ⛔ During critical business periods
- ⛔ Without testing in staging first
- ⛔ If lacking rollback plan
- ⛔ If team unfamiliar with both implementations

---

## Compatibility

### Protocol Compatibility ✅

**Version**: Both use Protocol Version 6

| Feature | C++ ET | Rust ET | Compatible? |
|---------|--------|---------|-------------|
| Protocol Version | 6 | 6 | ✅ Yes |
| Message Format | Protobuf | Protobuf | ✅ Yes |
| Encryption | XSalsa20-Poly1305 | XSalsa20-Poly1305 | ✅ Yes |
| Packet Framing | 4-byte BE length | 4-byte BE length | ✅ Yes |
| Nonce Handling | MSB-based | MSB-based | ✅ Yes |

**Interoperability**: Rust and C++ implementations can communicate seamlessly.

### Feature Parity

| Feature | C++ ET | Rust ET | Status |
|---------|--------|---------|--------|
| Basic terminal session | ✅ | ✅ | Complete |
| SSH integration | ✅ | ✅ | Complete |
| Encryption | ✅ | ✅ | Complete |
| Configuration file | ❌ | ✅ | Rust adds this |
| Port forwarding | ✅ | ⏳ | Planned |
| Jumphosts (multi-hop) | ✅ | ⏳ | Planned |
| Telemetry | Limited | ⏳ | Planned |

**Current State**: Core functionality complete, advanced features in progress.

### Binary Compatibility

**Not Binary Compatible**: Rust and C++ are separate binaries

**Migration Approach**: Side-by-side deployment, not in-place replacement

### Configuration Compatibility

**C++ ET**: No configuration file (command-line only)

**Rust ET**: TOML configuration file + command-line

**Migration**: Rust configuration is additive, not required. All C++ command-line workflows work in Rust.

---

## Migration Strategies

### 1. Greenfield Deployment (New Infrastructure)

**Best For**: New servers, new environments

**Approach**:
- Deploy Rust ET from the start
- No migration complexity
- Clean implementation

**Steps**:
1. Install Rust ET on new servers
2. Configure as per DEPLOYMENT_GUIDE.md
3. Users connect normally

**Pros**:
- ✅ Simplest approach
- ✅ No migration risk
- ✅ Clean start

**Cons**:
- ❌ Only for new infrastructure

---

### 2. Gradual Migration (Recommended)

**Best For**: Production environments with many servers

**Approach**:
- Migrate servers incrementally
- Test each phase before proceeding
- Maintain rollback capability

**Timeline**: 4-8 weeks for full migration

**Steps**:
1. **Week 1**: Pilot (1-2 non-critical servers)
2. **Week 2-3**: Expand (10% of servers)
3. **Week 4-6**: Majority (80% of servers)
4. **Week 7-8**: Complete (100% of servers)

**Pros**:
- ✅ Low risk
- ✅ Can pause/rollback at any stage
- ✅ Learn as you go

**Cons**:
- ❌ Takes longer
- ❌ Two implementations in production temporarily

---

### 3. Blue-Green Deployment

**Best For**: Environments with load balancers, clustered deployments

**Approach**:
- Deploy Rust ET in parallel (green)
- Switch traffic to Rust (cutover)
- Keep C++ as backup (blue)

**Steps**:
1. Deploy Rust servers (green)
2. Test thoroughly
3. Switch DNS/load balancer to green
4. Monitor
5. Decommission C++ servers (blue) after burn-in period

**Pros**:
- ✅ Fast cutover
- ✅ Easy rollback
- ✅ Minimal downtime

**Cons**:
- ❌ Requires double resources temporarily
- ❌ Need load balancer or DNS control

---

### 4. Mixed Deployment

**Best For**: Testing compatibility, gradual adoption

**Approach**:
- Run both C++ and Rust servers simultaneously
- Users can connect to either
- Migrate users gradually

**Steps**:
1. Install Rust ET alongside C++ ET (different ports)
2. Some users use Rust, some use C++
3. Gradually move users to Rust
4. Decommission C++ when all migrated

**Pros**:
- ✅ Very low risk
- ✅ Users can choose
- ✅ Proves interoperability

**Cons**:
- ❌ Complex management (two systems)
- ❌ Longer migration period
- ❌ Potential confusion

---

## Pre-Migration Checklist

### Technical Assessment

- [ ] Inventory of all servers running C++ ET
- [ ] Document current configuration (ports, passkeys, etc.)
- [ ] Review current usage patterns
- [ ] Identify critical vs non-critical servers
- [ ] Check for custom patches/modifications
- [ ] Verify Rust toolchain availability or plan installation

### Infrastructure Readiness

- [ ] Staging environment available for testing
- [ ] Monitoring/alerting configured
- [ ] Log aggregation set up
- [ ] Firewall rules documented
- [ ] Backup/recovery procedures tested

### Team Readiness

- [ ] Team trained on Rust ET differences
- [ ] Migration plan reviewed and approved
- [ ] Rollback procedures documented
- [ ] On-call schedule for migration period
- [ ] Communication plan for users

### Documentation

- [ ] Current C++ ET configuration documented
- [ ] Rust ET deployment plan written
- [ ] Rollback procedures documented
- [ ] User communication drafted
- [ ] Post-migration validation checklist prepared

---

## Migration Paths

### Path A: Server-by-Server Migration

**Timeline**: 2-6 weeks

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ All servers  │     │ Mixed        │     │ All servers  │
│ running C++  │────▶│ C++ + Rust   │────▶│ running Rust │
└──────────────┘     └──────────────┘     └──────────────┘
   Week 0               Week 1-5              Week 6
```

**Best For**: Standard deployments, < 50 servers

**Steps**:
1. Select pilot servers (1-2 non-critical)
2. Deploy Rust ET
3. Test for 48 hours
4. Add more servers (10%)
5. Test for 1 week
6. Continue until complete

---

### Path B: Environment-by-Environment Migration

**Timeline**: 3-8 weeks

```
┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐
│   Dev    │   │ Staging  │   │   Prod   │   │ Complete │
│  Rust    │──▶│  Rust    │──▶│  Rust    │──▶│  All     │
│ C++ off  │   │ C++ off  │   │ C++ off  │   │  Rust    │
└──────────┘   └──────────┘   └──────────┘   └──────────┘
  Week 1        Week 2-3       Week 4-7       Week 8
```

**Best For**: Separate dev/staging/prod environments

**Steps**:
1. Migrate development first
2. Learn and iterate
3. Migrate staging
4. Validate thoroughly
5. Migrate production
6. Decommission C++

---

### Path C: Parallel Deployment (No Migration)

**Timeline**: Indefinite

```
┌──────────────────┐
│   C++ ET         │  (Existing users, existing workflows)
│   Port 2022      │
├──────────────────┤
│   Rust ET        │  (New users, new features)
│   Port 2024      │
└──────────────────┘
```

**Best For**: Organizations wanting both options

**Steps**:
1. Install Rust ET on different port
2. New users use Rust
3. Existing users stay on C++
4. No forced migration

---

## Step-by-Step Migration

### Phase 1: Preparation (Week 1)

#### Step 1.1: Inventory Current Deployment

```bash
# List all servers running C++ ET
for server in $(cat servers.txt); do
    ssh $server "ps aux | grep -E 'etserver\s' | grep -v grep"
done

# Document current configuration
for server in $(cat servers.txt); do
    echo "=== $server ==="
    ssh $server "sudo netstat -tlnp | grep etserver"
    ssh $server "cat /etc/et/passkey 2>/dev/null | wc -c"
done
```

#### Step 1.2: Set Up Staging Environment

```bash
# Install Rust ET in staging
# See DEPLOYMENT_GUIDE.md for detailed instructions

# Install on staging server
scp et-rust/target/release/* staging-server:/tmp/
ssh staging-server "sudo cp /tmp/etserver-prod /usr/local/bin/"
ssh staging-server "sudo cp /tmp/etterminal-rs /usr/local/bin/"
ssh staging-server "sudo cp /tmp/et-rs /usr/local/bin/"
```

#### Step 1.3: Test in Staging

```bash
# Start Rust server in staging
ssh staging-server "sudo systemctl start etserver-prod"

# Test connection
et-rs staging-server

# Run automated test suite
cd et-rust
./run-e2e-tests.sh
```

---

### Phase 2: Pilot Deployment (Week 2)

#### Step 2.1: Select Pilot Servers

**Criteria**:
- Non-critical to business
- Representative workload
- Good monitoring
- Easy rollback

**Example**:
```bash
# Pilot servers
PILOT_SERVERS=(
    "dev-server-01.example.com"
    "qa-server-01.example.com"
)
```

#### Step 2.2: Deploy to Pilot Servers

```bash
# For each pilot server
for server in "${PILOT_SERVERS[@]}"; do
    echo "Deploying to $server..."

    # 1. Install binaries
    scp et-rust/target/release/etserver-prod $server:/tmp/
    scp et-rust/target/release/etterminal-rs $server:/tmp/
    ssh $server "sudo mv /tmp/etserver-prod /usr/local/bin/"
    ssh $server "sudo mv /tmp/etterminal-rs /usr/local/bin/"
    ssh $server "sudo chmod +x /usr/local/bin/etserver-prod"
    ssh $server "sudo chmod +x /usr/local/bin/etterminal-rs"

    # 2. Copy passkey (same as C++ for compatibility)
    scp /etc/et/passkey $server:/tmp/passkey
    ssh $server "sudo mv /tmp/passkey /etc/et/passkey"
    ssh $server "sudo chmod 600 /etc/et/passkey"

    # 3. Install systemd service
    scp et-rust/etserver-prod.service $server:/tmp/
    ssh $server "sudo mv /tmp/etserver-prod.service /etc/systemd/system/"
    ssh $server "sudo systemctl daemon-reload"

    # 4. Stop C++ server
    ssh $server "sudo systemctl stop etserver"

    # 5. Start Rust server
    ssh $server "sudo systemctl start etserver-prod"

    # 6. Verify
    ssh $server "sudo systemctl status etserver-prod"
    ssh $server "sudo netstat -tlnp | grep 2022"

    echo "✓ Deployed to $server"
done
```

#### Step 2.3: Monitor Pilot Servers

```bash
# Monitor logs
for server in "${PILOT_SERVERS[@]}"; do
    ssh $server "sudo journalctl -u etserver-prod -f" &
done

# Check metrics every hour
while true; do
    for server in "${PILOT_SERVERS[@]}"; do
        echo "=== $server ==="
        ssh $server "ps aux | grep etserver-prod"
        ssh $server "sudo netstat -an | grep :2022 | wc -l"
    done
    sleep 3600
done
```

#### Step 2.4: Validation Tests

```bash
# Test connectivity
for server in "${PILOT_SERVERS[@]}"; do
    echo "Testing $server..."
    timeout 30 et-rs $server "echo 'Migration test successful'"
    if [ $? -eq 0 ]; then
        echo "✓ $server OK"
    else
        echo "✗ $server FAILED"
    fi
done
```

---

### Phase 3: Gradual Rollout (Week 3-5)

#### Step 3.1: Expand to 10% of Servers

```bash
# Calculate 10%
TOTAL_SERVERS=$(wc -l < servers.txt)
BATCH_SIZE=$((TOTAL_SERVERS / 10))

# Select next batch
head -n $BATCH_SIZE servers.txt > batch1.txt

# Deploy using same script as pilot
for server in $(cat batch1.txt); do
    # ... same deployment steps ...
done
```

#### Step 3.2: Monitor for 1 Week

**Daily Checks**:
- Connection success rate
- Error logs
- Resource usage
- User feedback

**Success Criteria** (before proceeding):
- ✅ No critical errors
- ✅ Connection success rate > 99%
- ✅ No performance degradation
- ✅ No user complaints

#### Step 3.3: Continue Rolling Out

**Batches**:
1. Week 2: 10% (first batch)
2. Week 3: 30% (cumulative)
3. Week 4: 60% (cumulative)
4. Week 5: 100% (complete)

**Pause Conditions**:
- ⚠️ Error rate > 1%
- ⚠️ User complaints
- ⚠️ Unexpected issues
- ⚠️ Resource problems

---

### Phase 4: Completion (Week 6)

#### Step 4.1: Migrate Remaining Servers

```bash
# Final batch
comm -23 <(sort servers.txt) <(sort migrated.txt) > final-batch.txt

# Deploy
for server in $(cat final-batch.txt); do
    # ... same deployment steps ...
done
```

#### Step 4.2: Decommission C++ ET

```bash
# For each server (after Rust ET stable for 2+ weeks)
for server in $(cat servers.txt); do
    ssh $server "sudo systemctl disable etserver"  # C++ service
    ssh $server "sudo apt-get remove et"  # or equivalent
done
```

#### Step 4.3: Clean Up

```bash
# Remove old C++ binaries
for server in $(cat servers.txt); do
    ssh $server "sudo rm -f /usr/bin/et /usr/bin/etserver /usr/bin/etterminal"
done

# Keep C++ binaries backed up for 90 days
tar czf et-cpp-backup-$(date +%Y%m%d).tar.gz /usr/bin/et*
```

---

## Testing and Validation

### Pre-Migration Testing (Staging)

**Test Suite**:
```bash
# 1. Basic connectivity
et-rs staging-server "echo test"

# 2. Interactive session
et-rs staging-server
# Run commands, test terminal

# 3. Multiple connections
for i in {1..5}; do
    et-rs staging-server "echo Connection $i" &
done
wait

# 4. Long-running session
et-rs staging-server
# Leave running for 24 hours

# 5. Automated test suite
cd et-rust
./run-e2e-tests.sh
```

### Post-Migration Validation (Production)

**Checklist**:
- [ ] Server process running
- [ ] Port listening
- [ ] Can establish connection
- [ ] Interactive terminal works
- [ ] Terminal resize works
- [ ] Can disconnect cleanly
- [ ] Logs show no errors
- [ ] Resource usage normal

**Validation Script**:
```bash
#!/bin/bash
# validate-migration.sh

SERVER=$1

echo "Validating $SERVER..."

# 1. Check process
ssh $SERVER "pgrep -f etserver-prod" || { echo "✗ Server not running"; exit 1; }
echo "✓ Server running"

# 2. Check port
ssh $SERVER "sudo netstat -tln | grep ':2022.*LISTEN'" || { echo "✗ Port not listening"; exit 1; }
echo "✓ Port listening"

# 3. Test connection
timeout 30 et-rs $SERVER "echo test" > /dev/null 2>&1 || { echo "✗ Connection failed"; exit 1; }
echo "✓ Connection successful"

# 4. Check logs
ERRORS=$(ssh $SERVER "sudo journalctl -u etserver-prod --since '1 hour ago' -p err | wc -l")
if [ $ERRORS -gt 0 ]; then
    echo "⚠ $ERRORS errors in last hour"
else
    echo "✓ No errors in logs"
fi

echo "✓ Validation passed for $SERVER"
```

---

## Rollback Procedures

### When to Rollback

**Immediate Rollback**:
- ⛔ Cannot establish connections
- ⛔ Data corruption or loss
- ⛔ Security vulnerability discovered
- ⛔ Critical functionality broken

**Planned Rollback**:
- ⚠️ High error rate (> 5%)
- ⚠️ Performance degradation
- ⚠️ Excessive resource usage
- ⚠️ User complaints

### Rollback Procedure

**Single Server**:
```bash
#!/bin/bash
# rollback-server.sh

SERVER=$1

echo "Rolling back $SERVER to C++ ET..."

# 1. Stop Rust server
ssh $SERVER "sudo systemctl stop etserver-prod"

# 2. Start C++ server
ssh $SERVER "sudo systemctl start etserver"

# 3. Verify
ssh $SERVER "sudo systemctl status etserver"
ssh $SERVER "ps aux | grep etserver | grep -v grep"

echo "✓ Rolled back $SERVER"
```

**All Servers** (emergency):
```bash
#!/bin/bash
# emergency-rollback.sh

for server in $(cat migrated.txt); do
    echo "Rolling back $server..."
    ssh $server "sudo systemctl stop etserver-prod && sudo systemctl start etserver" &
done
wait

echo "Emergency rollback complete"
```

### Post-Rollback

1. **Document the Issue**:
   - What went wrong?
   - Error messages?
   - Logs?

2. **Analyze Root Cause**:
   - Code bug?
   - Configuration issue?
   - Infrastructure problem?

3. **Plan Re-Migration**:
   - Fix the issue
   - Test in staging
   - Plan new migration date

---

## Common Migration Scenarios

### Scenario 1: Small Organization (< 10 servers)

**Recommended Strategy**: Blue-Green

**Timeline**: 1 week

**Steps**:
1. Deploy Rust ET on all servers (different port)
2. Test thoroughly for 2 days
3. Switch all users to Rust (update port config)
4. Monitor for 3 days
5. Decommission C++ if no issues

**Risk Level**: Low (easy rollback)

---

### Scenario 2: Medium Organization (10-100 servers)

**Recommended Strategy**: Gradual Migration

**Timeline**: 4 weeks

**Steps**:
1. Week 1: Pilot (2 servers)
2. Week 2: 20% (20 servers)
3. Week 3: 60% (60 servers)
4. Week 4: 100% (all servers)

**Risk Level**: Low (pause/rollback at any stage)

---

### Scenario 3: Large Organization (100+ servers)

**Recommended Strategy**: Environment-by-Environment

**Timeline**: 8 weeks

**Steps**:
1. Weeks 1-2: Development environment
2. Weeks 3-4: Staging environment
3. Weeks 5-8: Production (in batches)

**Risk Level**: Very Low (extensive testing)

---

### Scenario 4: Multi-Datacenter Deployment

**Recommended Strategy**: Datacenter-by-Datacenter

**Timeline**: 6 weeks

**Steps**:
1. Week 1: DC1 (pilot datacenter)
2. Weeks 2-3: DC2
3. Weeks 4-5: DC3
4. Week 6: Remaining DCs

**Risk Level**: Low (isolated to datacenters)

---

## Troubleshooting

### Issue: Connection Fails After Migration

**Symptoms**:
- `et-rs` cannot connect to server
- "Connection refused" error

**Diagnosis**:
```bash
# Check server is running
ssh server "ps aux | grep etserver-prod"

# Check port is listening
ssh server "sudo netstat -tln | grep 2022"

# Check firewall
ssh server "sudo ufw status | grep 2022"
```

**Solutions**:
1. Server not running → `sudo systemctl start etserver-prod`
2. Port not listening → Check bind address in service file
3. Firewall blocking → `sudo ufw allow 2022/tcp`

**Reference**: `TROUBLESHOOTING_GUIDE.md` Issue #1

---

### Issue: "Client not registered" Error

**Symptoms**:
- Connection establishes but immediately rejected
- "Client is not registered" in logs

**Cause**: Passkey mismatch

**Diagnosis**:
```bash
# Check server passkey
ssh server "sudo cat /etc/et/passkey | wc -c"  # Should be 32+ bytes

# Check etterminal is using correct passkey
ssh server "ps aux | grep etterminal-rs"  # Check --passkey argument
```

**Solution**:
```bash
# Ensure same passkey on server and in etterminal launch
# Usually passed via environment in SSH
```

**Reference**: `TROUBLESHOOTING_GUIDE.md` Issue #3

---

### Issue: Performance Degradation

**Symptoms**:
- Slower response times
- High CPU usage

**Diagnosis**:
```bash
# Check resource usage
ssh server "top -b -n 1 | grep etserver-prod"

# Check connection count
ssh server "sudo netstat -an | grep :2022 | grep ESTABLISHED | wc -l"

# Check memory
ssh server "free -h"
```

**Solutions**:
1. Too many connections → Increase server resources
2. Memory leak (unlikely in Rust) → Restart server
3. CPU usage high → Check for runaway processes

---

### Issue: Rollback Needed

**See**: [Rollback Procedures](#rollback-procedures)

---

## Post-Migration

### Monitoring for First 30 Days

**Daily Checks**:
- [ ] Server uptime
- [ ] Connection success rate
- [ ] Error logs
- [ ] Resource usage (CPU, memory)
- [ ] User feedback

**Weekly Reviews**:
- [ ] Review all incidents
- [ ] Analyze trends
- [ ] Update documentation
- [ ] Plan improvements

### Long-Term Maintenance

**Monthly**:
- [ ] Review security advisories
- [ ] Update dependencies (`cargo update`)
- [ ] Review and rotate passkeys
- [ ] Audit access logs

**Quarterly**:
- [ ] Performance benchmarking
- [ ] Capacity planning
- [ ] Review and update documentation
- [ ] Team training on new features

### Success Metrics

**Track**:
- Uptime (target: 99.9%)
- Connection success rate (target: 99.9%)
- Mean time between failures (MTBF)
- Mean time to recovery (MTTR)
- User satisfaction

**Celebrate Success** 🎉:
- Migration complete
- No major issues
- Users happy
- Team confident

---

## Appendix: Quick Reference

### Migration Decision Tree

```
Do you have < 10 servers?
├─ Yes → Use Blue-Green deployment (1 week)
└─ No  → Do you have separate dev/staging/prod?
    ├─ Yes → Use Environment-by-Environment (8 weeks)
    └─ No  → Use Gradual Migration (4-6 weeks)
```

### Command Cheat Sheet

```bash
# Deploy Rust ET
scp target/release/etserver-prod server:/tmp/
ssh server "sudo mv /tmp/etserver-prod /usr/local/bin/"
ssh server "sudo systemctl start etserver-prod"

# Check status
ssh server "sudo systemctl status etserver-prod"
ssh server "sudo journalctl -u etserver-prod -f"

# Rollback
ssh server "sudo systemctl stop etserver-prod && sudo systemctl start etserver"

# Test connection
et-rs server "echo test"
```

### File Locations

| File | C++ ET | Rust ET |
|------|--------|---------|
| Server binary | `/usr/bin/etserver` | `/usr/local/bin/etserver-prod` |
| Client binary | `/usr/bin/et` | `/usr/local/bin/et-rs` |
| Terminal binary | `/usr/bin/etterminal` | `/usr/local/bin/etterminal-rs` |
| Passkey | `/etc/et/passkey` | `/etc/et/passkey` (same) |
| Config | N/A | `~/.et/config.toml` |
| Logs | `/var/log/syslog` | `journalctl -u etserver-prod` |

---

## Conclusion

Migrating from C++ to Rust ET is a low-risk, incremental process. With proper planning, testing, and monitoring, the migration can be completed smoothly with zero downtime.

**Key Takeaways**:
1. ✅ Protocol compatibility ensures seamless migration
2. ✅ Gradual approach minimizes risk
3. ✅ Rollback is always available
4. ✅ Thorough testing validates success
5. ✅ Benefits outweigh migration effort

**Need Help?**
- Documentation: See `DEPLOYMENT_GUIDE.md`, `TROUBLESHOOTING_GUIDE.md`
- Issues: Report at GitHub
- Community: [Community forum/chat if applicable]

**Good luck with your migration!** 🚀

---

*ET Rust Migration Guide v1.0*
*Last Updated: 2025-11-16*
