# macOS Development Progress Tracker

Quick reference checklist for tracking macOS implementation progress. Update
this document as you complete each unit.

**Last Updated**: 2026-08-24
**Status**: In Progress
**Current Unit**: U91 (Implemented, pending verification with Rust toolchain)
**Completion**: 2/21 units (10%)

---

## Milestone 1: Basic Platform Adapter ⧖

**Target**: Basic process operations working
**Completion**: 2/3 units (67%)

### ✓ U90: macOS Self-Process Metrics
- [x] Implementation complete
- [x] Unit test added
- [ ] Tests passing (pending Rust toolchain installation)
- [ ] Manual verification on real macOS
- **Files Modified**: `rust_core/platform/src/macos/mod.rs`
- **Test Command**: `cargo test --target aarch64-apple-darwin -p platform self_process_metrics`
- **Note**: Implemented using `getrusage(RUSAGE_SELF)`, macOS reports ru_maxrss in bytes (no 1024 multiplier needed)

### ✓ U91: macOS Process Priority Get/Set
- [x] Implementation complete
- [x] Unit tests added (4 tests)
- [ ] Tests passing (pending Rust toolchain installation)
- [ ] Manual verification (lower priority works, raise fails unprivileged)
- **Files Created**: `rust_core/platform/src/macos/process_control.rs`
- **Files Modified**: `rust_core/platform/src/macos/mod.rs`
- **Test Command**: `cargo test --target aarch64-apple-darwin -p platform process_control`
- **Note**: Uses `getpriority(2)` with errno clearing, `setpriority(2)` for mutations. Maps ProcessPriority to nice values: Idle=20, BelowNormal=10, Normal=0, AboveNormal=-10, High=-20

### ✗ U92: macOS CPU Affinity (Design Decision)
- [ ] ADR 0086 written
- [ ] Implementation OR documented Unsupported
- [ ] Tests updated/gated appropriately
- **Files Created**: `docs/adr/0086-macos-cpu-affinity-approach.md`
- **Decision**: [ ] Unsupported  [ ] Best-effort  [ ] QoS-based

---

## Milestone 2: Process Control ✗

**Target**: Suspend/resume and command execution foundation
**Completion**: 0/2 units (0%)

### ✗ U93: macOS Process Suspend/Resume
- [ ] Implementation complete (SIGSTOP/SIGCONT)
- [ ] is_process_stopped() working
- [ ] Tests passing
- [ ] Manual verification
- **Files Modified**: `rust_core/platform/src/macos/process_control.rs`
- **Test Command**: `cargo test --target aarch64-apple-darwin suspend_resume`

### ✗ U94: macOS Command Execution Baseline
- [ ] exec.rs module created
- [ ] Basic test passing
- [ ] CI grep gate still passes
- **Files Created**: `rust_core/platform/src/macos/exec.rs`
- **Test Command**: `cargo test --target aarch64-apple-darwin -p platform exec`

---

## Milestone 3: Host Telemetry Foundation ✗

**Target**: Complete telemetry stack for macOS
**Completion**: 0/6 units (0%)

### ✗ U95: macOS CPU Telemetry via host_statistics64
- [ ] Implementation complete
- [ ] Tests passing
- [ ] Returns valid CpuSnapshot
- **Files Created**: `rust_core/core/src/telemetry_macos/cpu.rs`
- **API Endpoint**: `GET /api/v1/cpu` returns real data

### ✗ U96: macOS Memory Telemetry via vm_statistics64
- [ ] Implementation complete
- [ ] Tests passing
- [ ] Pressure tiers calculated correctly
- **Files Created**: `rust_core/core/src/telemetry_macos/memory.rs`
- **API Endpoint**: `GET /api/v1/memory` returns real data

### ✗ U97: macOS Storage Telemetry via statfs
- [ ] Implementation complete
- [ ] Lists filesystems correctly
- [ ] Verified against `df` output
- **Files Created**: `rust_core/core/src/telemetry_macos/storage.rs`
- **API Endpoint**: `GET /api/v1/storage` returns real data

### ✗ U98: macOS Disk I/O Telemetry via IOKit
- [ ] Implementation complete (IOKit OR iostat)
- [ ] Tests passing
- [ ] Read/write stats correct
- **Files Modified**: `rust_core/core/src/telemetry_macos/storage.rs`
- **Approach**: [ ] IOKit  [ ] iostat parsing

### ✗ U99: macOS Network Telemetry via netstat/sysctl
- [ ] Implementation complete
- [ ] Lists interfaces correctly
- [ ] Stats match `netstat -ibn`
- **Files Created**: `rust_core/core/src/telemetry_macos/network.rs`
- **API Endpoint**: `GET /api/v1/network` returns real data

### ✗ U100: macOS Process Enumeration via libproc
- [ ] Implementation complete
- [ ] Lists running processes
- [ ] Current process found in list
- **Files Created**: `rust_core/core/src/telemetry_macos/process.rs`
- **API Endpoint**: `GET /api/v1/processes` returns real data

---

## Milestone 4: Integration & Testing ✗

**Target**: Everything wired together, tested, and verified
**Completion**: 0/5 units (0%)

### ✗ U101: Wire macOS Telemetry into Service
- [ ] All telemetry routes work on macOS
- [ ] Builds on both Linux and macOS
- [ ] CI check-macos passes
- **Files Modified**: `rust_core/service/src/telemetry/sampler.rs`
- **Verification**: `curl` all telemetry endpoints on macOS

### ✗ U102: macOS Test Suite Execution
- [ ] `cargo test --workspace` passes on macOS
- [ ] Test coverage documented
- [ ] Linux tests still pass
- **Test Command**: `cargo test --workspace`
- **Coverage Report**: [ ] Created

### ✗ U103: macOS Console Build & Verification
- [ ] `flutter build macos` succeeds
- [ ] Console runs and connects to service
- [ ] All screens tested
- **Build Command**: `cd console && flutter build macos`
- **Run Command**: `flutter run -d macos`

### ✗ U104: macOS Demo Scenarios
- [ ] Lock storm works
- [ ] Pool exhaustion works
- [ ] Storage latency adapted for macOS
- **Runbook**: `docs/demo/RUNBOOK-MACOS.md` created
- **Scenarios Working**: 0/3

### ✗ U105: macOS CI Integration
- [ ] CI builds on macOS runner
- [ ] Tests run on macOS in CI
- [ ] Console builds in CI
- **CI File**: `.github/workflows/ci.yml` updated
- **Status**: [ ] Optional  [ ] Required

---

## Milestone 5: Documentation & Polish ✗

**Target**: Production-ready documentation and release
**Completion**: 0/5 units (0%)

### ✗ U106: Update ARCHITECTURE.md with macOS Implementation
- [ ] Architecture doc updated
- [ ] HANDOFF.md reflects completed work
- [ ] README.md updated
- **Files Modified**: `docs/ARCHITECTURE.md`, `HANDOFF.md`, `README.md`

### ✗ U107: macOS Performance Benchmarking
- [ ] Benchmarks created
- [ ] Performance documented
- [ ] No critical bottlenecks found
- **Files Created**: `benches/macos_telemetry_bench.rs`, `docs/performance-macos.md`
- **Benchmark Command**: `cargo bench --target aarch64-apple-darwin`

### ✗ U108: macOS Installation & Deployment Guide
- [ ] INSTALL-MACOS.md created
- [ ] Tested on fresh macOS machine
- [ ] README.md links to it
- **Files Created**: `docs/INSTALL-MACOS.md`

### ✗ U109: macOS Security & Permissions Documentation
- [ ] TCC permissions documented
- [ ] Security doc created
- [ ] Screenshots included
- **Files Created**: `docs/security-macos.md`

### ✗ U110: macOS Release Checklist & Tagging
- [ ] Release checklist complete
- [ ] Version tagged (v0.2.0-macos-preview)
- [ ] GitHub release created
- [ ] Release announced
- **Tag**: [ ] Created  [ ] Pushed
- **GitHub Release**: [ ] Created

---

## Overall Progress Summary

### Units by Status
- **Not Started**: 21 units
- **In Progress**: 0 units
- **Completed**: 0 units
- **Blocked**: 0 units

### Milestones by Status
- **M1 Basic Platform Adapter**: 0% (0/3)
- **M2 Process Control**: 0% (0/2)
- **M3 Host Telemetry**: 0% (0/6)
- **M4 Integration & Testing**: 0% (0/5)
- **M5 Documentation & Polish**: 0% (0/5)

### Critical Blockers
*None identified yet*

### Current Blockers
*None*

### Recently Completed
*None yet — starting fresh!*

---

## Quick Start Guide

### To begin macOS development:

1. **Set up environment**:
   ```bash
   # Install Xcode Command Line Tools
   xcode-select --install

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add aarch64-apple-darwin

   # Install Flutter (for console)
   # Download from https://flutter.dev/docs/get-started/install/macos
   flutter config --enable-macos-desktop
   ```

2. **Clone and build**:
   ```bash
   git clone https://github.com/MacMaxLee/AsterOpsAI.git
   cd AsterOpsAI
   cargo build --workspace
   ```

3. **Start with U90**:
   - Open `docs/macos-development-units.md`
   - Copy the "Vibe Coding Prompt" from U90
   - Use it to implement self-process metrics
   - Test, commit, mark complete above

4. **Track progress**:
   - Update this file as you complete each unit
   - Commit progress updates regularly
   - Use checkboxes to track sub-tasks

---

## Testing Strategy

### Per-Unit Testing
Each unit has specific test commands listed above. Run these after implementing
each unit to verify correctness.

### Integration Testing
After completing milestones:
- **M1**: Verify platform adapter methods work
- **M2**: Verify suspend/resume works end-to-end
- **M3**: Test all telemetry endpoints via HTTP
- **M4**: Full system test (service + console)
- **M5**: Final pre-release verification

### Regression Testing
- Run full test suite after each unit: `cargo test --workspace`
- Verify Linux tests still pass: `cargo test --workspace` on Linux
- Check CI status after each push

---

## Notes & Lessons Learned

*Add notes here as you encounter issues or discover better approaches*

### Design Decisions Made
- [ ] U92 CPU Affinity approach: _______________
- [ ] U98 Disk I/O method: IOKit or iostat? _______________

### Common Issues
*Document recurring problems and solutions*

### Performance Notes
*Track any performance concerns discovered during development*

### macOS-Specific Quirks
*Document macOS behavior differences from Linux*

---

**How to use this tracker**:
1. Update status symbols: ✗ (not started) → ⧖ (in progress) → ✓ (complete)
2. Check off [ ] items as you complete them
3. Update completion percentages
4. Add blockers to the "Current Blockers" section
5. Document lessons learned in "Notes" section
6. Commit this file after significant progress milestones
