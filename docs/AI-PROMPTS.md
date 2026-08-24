# AsterOpsAI macOS Development - AI Coding Prompts

Ready-to-use AI prompts for implementing macOS support. Copy and paste these
into Claude Code or your AI assistant of choice.

---

## Milestone 1: Basic Platform Adapter

### U90: macOS Self-Process Metrics

```
I'm implementing macOS self-process metrics for AsterOpsAI. The goal is to
replace the stub in rust_core/platform/src/macos/mod.rs with a real
implementation.

Context:
- Linux uses libc::getrusage(RUSAGE_SELF) to get RSS and CPU time
- macOS reports ru_maxrss in BYTES (Linux uses kilobytes)
- I need to sum ru_utime + ru_stime for total CPU time
- Return ProcessSelfMetrics { rss_bytes, cpu_time }

Please:
1. Show me the exact implementation for self_process_metrics()
2. Include proper unsafe block with SAFETY comment
3. Add a unit test that verifies RSS > 0 and cpu_time > 0
4. Make sure error handling returns CapabilityError::Io on failure

Reference the Linux implementation in rust_core/platform/src/linux/mod.rs
(the get_rusage_self() function) but adjust for macOS's byte-based ru_maxrss.
```

---

### U91: macOS Process Priority Get/Set

```
Implement macOS process priority control for AsterOpsAI using setpriority(2).

Context:
- ProcessPriority enum: Idle, BelowNormal, Normal, AboveNormal, High
- Map to nice values: 20, 10, 0, -10, -20
- Use PRIO_PROCESS with pid
- getpriority can return -1 as a valid value, so check errno
- Raising priority without privilege should return PermissionRequired

Please:
1. Create rust_core/platform/src/macos/process_control.rs
2. Implement get_priority(pid: u32) -> Result<ProcessPriority, CapabilityError>
3. Implement set_priority(pid: u32, priority: ProcessPriority) -> Result<(), CapabilityError>
4. Add unit tests for getting self priority, lowering (should succeed), raising (should fail unprivileged)
5. Wire it into mod.rs

Reference Linux's platform/src/linux/process_control.rs for the pattern, but
use setpriority/getpriority instead of Linux's scheduler-specific calls.
```

---

### U92: macOS CPU Affinity (Design Decision)

```
Design the macOS CPU affinity strategy for AsterOpsAI.

Context:
- Linux has sched_setaffinity (hard mask: "only run on CPUs 0,2,4")
- macOS has thread_policy_set with THREAD_AFFINITY_POLICY (per-thread hint, scheduler may ignore)
- Windows has SetProcessAffinityMask (hard mask like Linux)
- The PlatformAdapter trait expects get/set_process_cpu_affinity returning CpuAffinityMask

Problem: macOS doesn't have process-level hard affinity masks.

Please:
1. Analyze the three options:
   A. Return Unsupported (honest about capability gap)
   B. Best-effort via thread_policy_set (enumerate threads, set affinity hint per thread)
   C. Map to QoS instead (different semantic)

2. Write docs/adr/0086-macos-cpu-affinity-approach.md with:
   - Context: why this is different from Linux/Windows
   - Decision: which option to choose
   - Consequences: what users should expect
   - Alternatives considered

3. If choosing A: show how to return Unsupported with ADR reference
4. If choosing B or C: outline the implementation approach

My recommendation: Start with Option A (Unsupported) unless there's a real
use case requiring best-effort affinity. Document it, ship macOS support
without it, revisit if users request it.
```

---

## Milestone 2: Process Control

### U93: macOS Process Suspend/Resume

```
Implement macOS process suspend/resume for AsterOpsAI using SIGSTOP/SIGCONT.

Context:
- Linux uses kill(pid, SIGSTOP) / kill(pid, SIGCONT) — same on macOS
- Linux checks /proc/[pid]/stat for 'T' state — macOS needs different approach
- Need is_process_stopped() to verify suspend actually worked

Options for is_stopped on macOS:
A. Use libproc: proc_pidinfo with PROC_PIDTASKINFO, check pti_status
B. Parse `ps -o state= -p [pid]` looking for "T"
C. Use sysctl with KERN_PROC for process state

Please:
1. Implement suspend/resume using libc::kill
2. Choose and implement is_stopped (recommend option B for simplicity)
3. Add error handling: ESRCH -> NotFound, EPERM -> PermissionRequired
4. Add unit test: spawn child, suspend, check stopped, resume, check not stopped
5. Add to rust_core/platform/src/macos/process_control.rs

Reference platform/src/linux/process_control.rs for the pattern.
```

---

### U94: macOS Command Execution Baseline

```
Create the macOS exec.rs module for AsterOpsAI command execution.

Context:
- Only platform/*/exec.rs modules can call std::process::Command (CI enforced)
- Linux's exec.rs has helper functions for common command patterns
- This is the foundation for future sysctl/ps/ioreg parsing

Please:
1. Create rust_core/platform/src/macos/exec.rs
2. Add a basic structure with doc comment explaining this is the only Command location
3. Add a test that runs /bin/echo hello and verifies output contains "hello"
4. Keep it minimal — just the foundation

Reference platform/src/linux/exec.rs for the pattern, but keep it simple for now.
```

---

## Milestone 3: Host Telemetry Foundation

### U95: macOS CPU Telemetry via host_statistics64

```
Implement macOS CPU telemetry using host_statistics64 Mach API.

Context:
- Linux uses /proc/stat parsing for CPU ticks
- macOS uses Mach host_statistics64(HOST_CPU_LOAD_INFO)
- Need both total CPU and per-core utilization
- Return contracts::CpuSnapshot (same wire type as Linux)

Please:
1. Create rust_core/core/src/telemetry_macos/cpu.rs
2. Define FFI bindings for host_cpu_load_info_data_t
3. Implement cpu_snapshot() -> Result<CpuSnapshot, TelemetryError>
4. Use host_statistics64 for system-wide, host_processor_info for per-core
5. Calculate utilization from tick deltas (need to store previous sample)
6. Add unit test that takes two samples and verifies valid percentages

Reference:
- contracts::CpuSnapshot structure
- Apple's host_statistics64 documentation
- Similar pattern to Linux's core/src/telemetry/cpu.rs
```

---

### U96: macOS Memory Telemetry via vm_statistics64

```
Implement macOS memory telemetry using vm_statistics64.

Context:
- Linux uses /proc/meminfo (MemTotal, MemAvailable, SwapTotal, SwapFree)
- macOS uses Mach vm_statistics64 for VM stats + sysctl for total RAM
- Need to calculate 4-tier pressure (None/Low/Medium/High)

Please:
1. Create rust_core/core/src/telemetry_macos/memory.rs
2. FFI bindings for vm_statistics64_data_t
3. Get total RAM via sysctl hw.memsize
4. Get VM stats via host_statistics64(HOST_VM_INFO64)
5. Calculate available = (free + inactive) * PAGE_SIZE
6. Implement 4-tier pressure based on available / total percentage
7. Return contracts::MemorySnapshot

Reference core/src/telemetry/memory.rs for the pressure tier logic.
```

---

### U97: macOS Storage Telemetry via statfs

```
Implement macOS storage telemetry using statfs.

Context:
- statfs is POSIX, similar on Linux and macOS
- Need to enumerate mount points (getfsstat on macOS vs /proc/mounts on Linux)
- Calculate capacity, used, available, pressure tier

Please:
1. Create rust_core/core/src/telemetry_macos/storage.rs
2. Use getfsstat to enumerate filesystems
3. Call statfs for each mount point
4. Skip virtual filesystems (f_type check or name-based filter)
5. Calculate pressure from used percentage
6. Return contracts::StorageSnapshot with Vec<FilesystemInfo>

Reference platform/src/linux/storage.rs for the VolumeCapacity pattern.
```

---

### U98: macOS Disk I/O Telemetry via IOKit

```
Implement macOS disk I/O stats using IOKit or iostat.

Context:
- Linux parses /proc/diskstats
- macOS can use IOKit IOBlockStorageDriver or parse `iostat -d` output
- Need read/write bytes and operation counts per disk

Options:
A. IOKit (more complex, direct API)
B. Parse `iostat -d -w 1 -c 1` output (simpler, less efficient)

Please:
1. Recommend which approach for v1 (I suggest iostat for speed)
2. If iostat: implement parsing in telemetry_macos/storage.rs using exec.rs
3. If IOKit: show FFI structure and IOServiceMatching setup
4. Return per-disk I/O metrics
5. Add unit test

Let's start with iostat parsing for simplicity, can optimize to IOKit later.
```

---

### U99: macOS Network Telemetry via netstat/sysctl

```
Implement macOS network telemetry via netstat or sysctl.

Context:
- Linux parses /proc/net/dev
- macOS can use `netstat -ibn` or sysctl net.link.generic
- Need per-interface bytes/packets in/out

Please:
1. Create rust_core/core/src/telemetry_macos/network.rs
2. Choose approach: netstat parsing (simpler) vs sysctl (more direct)
3. Implement interface enumeration and stats collection
4. Filter out loopback unless needed
5. Return contracts::NetworkSnapshot
6. Add unit test checking for at least one interface

Recommend netstat parsing for initial implementation.
```

---

### U100: macOS Process Enumeration via libproc

```
Implement macOS process enumeration using libproc.

Context:
- Linux parses /proc/[pid]/stat, /proc/[pid]/status, /proc/[pid]/cmdline
- macOS uses libproc: proc_listallpids + proc_pidinfo + proc_pidpath
- Need PID, name, memory, CPU, state for each process

Please:
1. Create rust_core/core/src/telemetry_macos/process.rs
2. FFI bindings for libproc (or use libproc crate if available)
3. Implement process_snapshot() that:
   - Lists all PIDs via proc_listallpids
   - Gets info via proc_pidinfo(PROC_PIDTASKINFO)
   - Gets path via proc_pidpath
   - Gets args via sysctl KERN_PROCARGS2
4. Return contracts::ProcessSnapshot
5. Add test: find current process in returned list

This is the biggest telemetry unit — break into helper functions.
```

---

## Milestone 4: Integration & Testing

### U101: Wire macOS Telemetry into Service

```
Wire macOS telemetry into the AsterOpsAI service HTTP API.

Context:
- service/src/telemetry/sampler.rs currently calls core::telemetry (Linux only)
- Need to conditionally compile: Linux uses core::telemetry, macOS uses core::telemetry_macos
- Same wire types (contracts::CpuSnapshot, etc.)

Please:
1. Update service/src/telemetry/sampler.rs with platform conditionals
2. Ensure routes still work on both platforms
3. Test plan: build on macOS, start service, curl each endpoint
4. Show the conditional compilation structure

Both platforms should expose identical HTTP APIs, just different implementations.
```

---

### U102: macOS Test Suite Execution

```
Run and fix the AsterOpsAI test suite on macOS.

Context:
- Full test suite developed on Linux
- Most business logic tests are cross-platform
- Some telemetry/platform tests are Linux-specific

Task:
1. Run `cargo test --workspace` on macOS
2. Document each failure:
   - Test name
   - Failure reason
   - Fix approach (gate as Linux-only, or fix implementation)
3. Apply fixes
4. Create a test coverage report showing:
   - Tests that pass on both platforms
   - Tests gated to Linux only
   - Tests gated to macOS only
   - Any missing test coverage

Expected: ~90% of tests should pass on macOS with minimal changes.
```

---

### U103: macOS Console Build & Verification

```
Build and verify the AsterOpsAI Flutter console on macOS.

Context:
- Console developed and tested on Linux desktop
- Flutter supports macOS desktop natively
- Unix socket transport should work identically

Task:
1. Set up Flutter macOS desktop support
2. Build console for macOS
3. Run console against macOS service
4. Test each screen/feature
5. Document any macOS-specific issues (fonts, rendering, etc.)
6. Create macOS build instructions for console/README.md

Expected: Console should work with zero or minimal changes.
```

---

### U104: macOS Demo Scenarios

```
Adapt AsterOpsAI demo scenarios for macOS.

Context:
- 3 Linux demo scenarios exist: lock-storm, pool-exhaustion, storage-latency
- Lock/pool scenarios are PostgreSQL-level (platform-agnostic)
- Storage latency uses cgroup v2 io.max (Linux-specific)

Task:
1. Test lock-storm.sh on macOS (should work as-is)
2. Test pool-exhaustion.sh on macOS (should work as-is)
3. Design macOS alternative for storage-latency:
   - Option A: dd with throttling
   - Option B: slow external drive
   - Option C: network mount with artificial latency
4. Write docs/demo/RUNBOOK-MACOS.md
5. Execute all scenarios, verify correlation engine works

Focus: Prove the correlation engine is truly cross-platform.
```

---

### U105: macOS CI Integration

```
Add macOS CI to AsterOpsAI GitHub Actions.

Context:
- Existing CI has check-macos job that only runs `cargo check`
- Need real build and test on macOS
- GitHub provides macos-latest runners

Task:
1. Update .github/workflows/ci.yml
2. Add macOS-specific jobs:
   - cargo build --workspace
   - cargo test --workspace
   - flutter build macos
3. Use caching for speed
4. Initially allow failures (continue-on-error: true)
5. Document path to making it required

Show the GitHub Actions YAML structure for macOS jobs.
```

---

## Milestone 5: Documentation & Polish

### U106: Update ARCHITECTURE.md with macOS Implementation

```
Document the macOS implementation in AsterOpsAI architecture docs.

Context:
- macOS support is now complete (or nearly complete)
- Need to update docs to reflect this
- HANDOFF.md was Linux→macOS transition, now need "current state" doc

Task:
1. Review existing docs for outdated Linux-only claims
2. Create/update docs/ARCHITECTURE.md with:
   - Platform abstraction overview
   - Linux implementation summary
   - macOS implementation summary
   - Cross-platform vs platform-specific components
3. Update README.md to list macOS as supported platform
4. Update HANDOFF.md §7 or add "macOS completion" section

Focus: Make it easy for next developer to understand platform abstraction.
```

---

### U107: macOS Performance Benchmarking

```
Benchmark AsterOpsAI macOS telemetry performance.

Context:
- Need to ensure telemetry is fast enough for real-time use
- Target: full snapshot in <50ms
- Compare macOS to Linux performance

Task:
1. Create benches/macos_telemetry_bench.rs using Criterion
2. Benchmark each telemetry module:
   - cpu_snapshot()
   - memory_snapshot()
   - storage_snapshot()
   - network_snapshot()
   - process_snapshot()
3. Run on real hardware, document results
4. Create docs/performance-macos.md with findings
5. Recommend optimizations if needed

Show Criterion benchmark structure for telemetry functions.
```

---

### U108: macOS Installation & Deployment Guide

```
Write macOS installation guide for AsterOpsAI.

Context:
- Linux uses XDG_RUNTIME_DIR for socket
- macOS should use ~/Library/Application Support or similar
- Need to cover developer setup (not production deployment yet)

Task:
1. Create docs/INSTALL-MACOS.md with:
   - Prerequisites (Xcode CLI tools, Rust, Flutter)
   - Building from source
   - Running the service
   - Running the console
   - Connecting to PostgreSQL
   - Verifying it works
   - Troubleshooting
2. Update README.md with macOS section
3. Test on fresh macOS VM or machine

Make it beginner-friendly — assume user knows macOS but not Rust/Flutter.
```

---

### U109: macOS Security & Permissions Documentation

```
Document macOS security and TCC permissions for AsterOpsAI.

Context:
- macOS has Transparency, Consent, and Control (TCC) for privacy
- Some operations trigger permission prompts
- Need to document what permissions are needed and why

Task:
1. Test AsterOpsAI on macOS and identify permission prompts:
   - Does process enumeration need Full Disk Access?
   - Does network telemetry trigger anything?
2. Create docs/security-macos.md explaining:
   - What permissions are needed
   - Why each is needed
   - How to grant them
   - What fails without them
3. Add screenshots of System Settings panels
4. Document graceful degradation (PermissionRequired errors)

Reference TRS §39 for TCC requirements already identified.
```

---

### U110: macOS Release Checklist & Tagging

```
Create release checklist and tag first macOS release of AsterOpsAI.

Context:
- First version with macOS support
- Should be tagged as preview/beta, not production
- Follows semver: 0.2.0 (minor version bump for new platform)

Task:
1. Create docs/RELEASE-CHECKLIST.md with all verification steps
2. Draft release notes for v0.2.0-macos-preview:
   - What's new (macOS support!)
   - Features available on macOS
   - Known limitations
   - Installation instructions
   - Breaking changes (if any)
3. Tag in git: `git tag -a v0.2.0-macos-preview -m "..."`
4. Create GitHub release from tag
5. Update README.md to mention current version

Show example release notes structure.
```

---

## How to Use These Prompts

1. **Copy the entire prompt** from the code block above
2. **Paste into your AI assistant** (Claude Code, ChatGPT, etc.)
3. **Review the generated code** carefully
4. **Test following the Definition of Done** from the full unit specification
5. **Iterate if needed** by providing feedback to the AI
6. **Commit when complete** and move to the next unit

## Tips for Best Results

- **Provide context**: If the AI asks questions, answer with specifics from the codebase
- **Reference existing code**: Point the AI to similar Linux implementations for patterns
- **Test incrementally**: Run tests after each change, don't batch multiple units
- **Ask for explanations**: If generated code is unclear, ask the AI to explain its approach
- **Iterate**: It's fine to go back and forth - AI coding is collaborative

## Customization

Feel free to modify these prompts based on:
- Your preferred implementation approach
- Additional context from your specific setup
- Lessons learned from previous units
- Feedback from code reviews

The prompts are starting points - adapt them to your needs!

---

**Ready to start? Begin with U90 and work your way through!** 🚀
