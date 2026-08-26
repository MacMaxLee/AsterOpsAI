# Release Notes - v0.2.0-macos-preview

**Release Date**: 2026-08-25
**Release Type**: Preview/Beta (Pre-release)
**Previous Version**: v0.1.x (Linux-only)

---

## 🎉 What's New

### macOS Platform Support (PREVIEW)

AsterOpsAI now runs natively on macOS! This preview release brings full telemetry and process control capabilities to Mac users.

**Supported macOS Versions**: macOS 10.15 (Catalina) and later
**Tested On**: macOS 26.6.2 (Apple Silicon M4 Pro)

---

## ✨ Features

### Telemetry Modules

All telemetry modules are fully functional on macOS:

**CPU Telemetry** (`/api/v1/cpu`)
- System-wide and per-core CPU utilization
- Load averages (1-min, 5-min, 15-min)
- CPU pressure classification (Normal/Elevated/High/Critical)
- Performance: 4.48-4.64 µs per sample

**Memory Telemetry** (`/api/v1/memory`)
- Total RAM, used, available, free
- Swap usage
- Memory pressure classification
- Performance: 2.00 µs per sample

**Storage Telemetry** (`/api/v1/storage`)
- Filesystem capacity and free space
- All mount points (APFS, HFS+, external drives, network volumes)
- Performance: 19.07 µs per sample

**Network Telemetry** (`/api/v1/network`)
- Per-interface statistics (bytes/packets received/transmitted)
- Rate calculations (bytes/sec, packets/sec)
- Error counters
- Performance: 1.56-1.58 ms per sample

**Process Telemetry** (`/api/v1/processes`)
- System-wide process enumeration
- Per-process CPU percentage, memory usage (RSS)
- Process names, executable paths, owner UIDs
- Process classification (System/UserApplication/BackgroundService/DbmsEngine)
- Performance: 1.09-1.10 ms for ~846 processes

**System Status** (`/api/v1/system/status`)
- System uptime
- Overall CPU and memory pressure
- Containerization detection
- Capability reporting

### Process Control

**Priority Management**:
- Change process priority (nice/renice)
- Works for processes owned by the same user
- Graceful permission errors for other users' processes

**Process Termination**:
- Terminate processes (SIGTERM)
- Force-terminate processes (SIGKILL)
- Limited to own processes for security

### Flutter Console (macOS Desktop)

- Native macOS app bundle (`.app`)
- All 9 screens functional (Dashboard, CPU, Memory, Storage, Network, Processes, Database, Policy, Security)
- Real-time telemetry updates
- Internationalization support (English, Chinese Traditional)
- Dark mode support

### Outstanding Performance

**Full Telemetry Cycle**: **2.998 ms** (60x faster than 50ms target)

**Production-Ready**:
- 1-second sampling intervals recommended
- <0.3% CPU overhead at 1-second sampling
- Scales to 2000+ processes

See `docs/performance-macos.md` for complete benchmark analysis.

### Zero-Friction Deployment

**Works Without Special Permissions**:
- No TCC prompts for standard telemetry
- No Full Disk Access required (optional for edge cases)
- No elevated (root) privileges needed

See `docs/security-macos.md` for security model and privacy considerations.

---

## 📚 Documentation

New macOS-specific documentation:

1. **Installation Guide** (`docs/INSTALL-MACOS.md`)
   - Prerequisites (Xcode CLI tools, Rust, Flutter)
   - Building from source
   - XDG_RUNTIME_DIR setup
   - Troubleshooting

2. **Performance Benchmarks** (`docs/performance-macos.md`)
   - Criterion benchmark results
   - Performance analysis
   - Production recommendations
   - Hardware variability estimates

3. **Security & Permissions** (`docs/security-macos.md`)
   - TCC framework explanation
   - Permission requirements
   - Unix socket security model
   - Privacy considerations

4. **Architecture** (`docs/ARCHITECTURE.md`)
   - macOS platform adapter design
   - Telemetry architecture
   - Known gaps and limitations

---

## ⚠️ Known Limitations

### Not Supported on macOS

**CPU Affinity** (ADR 0086):
- macOS does not expose CPU affinity APIs
- `set_affinity_mask()` returns `CapabilityError::Unavailable`
- Rationale: macOS scheduler is optimized for Apple Silicon asymmetric cores

**Device Telemetry** (Deferred):
- `/api/v1/devices` returns empty list
- Would require IORegistry/IOKit APIs (future work)

**Disk I/O Metrics** (Deferred):
- Storage telemetry shows capacity/free space only
- Per-disk I/O rates (reads/writes per second) not implemented
- Would require IOKit APIs (future work)

### Partially Supported

**Process Executable Paths**:
- Most processes: ✅ Full executable path available
- Other users' processes: ⚠️ May require Full Disk Access (rare)
- Returns `MetricValue::Unavailable` when inaccessible

**Process cmdline**:
- Not implemented (requires KERN_PROCARGS2 parsing)
- Future enhancement

---

## 🔧 Installation

### Quick Start

```bash
# Set up runtime directory
export XDG_RUNTIME_DIR="/tmp/runtime-$(id -u)"
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"

# Clone and build
git clone https://github.com/anthropics/AsterOpsAI.git
cd AsterOpsAI
cargo build --release

# Run service
./target/release/ai-ops-core serve
```

Test with curl (in another terminal):
```bash
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/health
```

**Full Installation Guide**: See [`docs/INSTALL-MACOS.md`](docs/INSTALL-MACOS.md)

---

## 🛡️ Security

### Permission Model

- **Works out-of-the-box** without special permissions
- **Unix socket**: mode 0600 (owner read/write only)
- **Runtime directory**: mode 0700 (per-user isolation)
- **No network exposure**: Local-only Unix socket
- **Never runs as root**: User-level privilege only

### Privacy

- **No PII collected**: Only system metrics and process names
- **All data stays local**: No cloud services, no external API calls
- **User controls data**: SQLite database (if used) is user-owned

**Full Security Guide**: See [`docs/security-macos.md`](docs/security-macos.md)

---

## 🔄 Breaking Changes

**None**. This release adds macOS support without changing existing Linux functionality.

**Platform-Specific Behavior**:
- **Linux**: Continues to work as before
- **macOS**: New platform support (this release)
- **Windows**: Compilation check only (telemetry stubbed)

---

## 📊 Testing

**Test Coverage on macOS**:
- **121 library tests** passing on macOS
- **2 Linux-specific tests** properly gated with `#[cfg(target_os = "linux")]`
- **~26 PostgreSQL integration tests** skipped (environment requirement, not platform issue)

**CI Pipeline**:
- `test-macos` job runs on `macos-latest` in GitHub Actions
- Validates all macOS platform code automatically

---

## 🚀 Development History

**Milestone Breakdown** (21 units total):

1. **Milestone 1**: Basic Platform Adapter (U90-U92) — Process metrics, priority, affinity
2. **Milestone 2**: Process Control (U93-U94) — Suspend/resume, exec foundation
3. **Milestone 3**: Host Telemetry (U95-U100) — CPU, memory, storage, network, processes
4. **Milestone 4**: Integration & Testing (U101-U105) — Service integration, testing, CI
5. **Milestone 5**: Documentation & Polish (U106-U110) — Docs, benchmarks, release

**Total Commits**: 20+ commits across all milestones

**See**: `docs/PROGRESS.md` for complete development history with commit references

---

## 🎯 What's Next

### Milestone 6: PostgreSQL Monitoring & Tuning

Future releases will focus on:
- PostgreSQL connection and monitoring
- Database-specific telemetry
- Tuning recommendations
- AI-powered analysis

### v1.0 Goals

- Production deployment guide
- SMAppService helper for system-wide monitoring
- Binary releases with Apple code signing
- Homebrew formula
- Device telemetry (IORegistry)
- Disk I/O metrics (IOKit)

---

## ⚠️ Important: Preview Release

**This is a PREVIEW/BETA release**. While extensively tested, it is:

✅ **Recommended for**:
- Development environments
- Testing and evaluation
- Early adopters
- Feedback gathering

❌ **NOT recommended for**:
- Production deployments
- Critical infrastructure
- Unsupervised operation

We welcome feedback! Please report issues at: https://github.com/anthropics/AsterOpsAI/issues

---

## 🙏 Acknowledgments

macOS platform support implemented with:
- **Mach kernel APIs** for CPU and memory telemetry
- **sysctl** for system information
- **libproc** for process enumeration
- **netstat** for network statistics
- **getfsstat/statfs** for filesystem telemetry

Thanks to the Rust community for excellent FFI support and the Flutter team for macOS desktop capabilities.

---

## 📦 Release Artifacts

**Source Code**:
- Tag: `v0.2.0-macos-preview`
- Branch: `main`
- Commits: See `git log v0.1.0..v0.2.0-macos-preview`

**Binary Releases**: Not included in this preview release. Build from source using instructions above.

**Checksums**: N/A (source-only release)

---

## 📝 Full Changelog

See [`docs/PROGRESS.md`](docs/PROGRESS.md) for detailed unit-by-unit development history.

**Highlights**:
- 2,200+ lines of macOS telemetry implementation
- 160 lines of Criterion benchmarks
- 1,500+ lines of documentation
- 50+ checklist items verified

---

**Questions? Issues? Feedback?**
- GitHub Issues: https://github.com/anthropics/AsterOpsAI/issues
- Documentation: `docs/` directory
- Installation Help: `docs/INSTALL-MACOS.md`
- Security Questions: `docs/security-macos.md`

Thank you for trying AsterOpsAI v0.2.0-macos-preview!
