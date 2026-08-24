# AsterOpsAI macOS Development Units

**Pre-read**: `HANDOFF.md`, `docs/mac-windows-transition-pack.md`, `CLAUDE.md`

This document defines the unit-by-unit plan for bringing full macOS support to
AsterOpsAI. Each unit follows the project's existing structure: GOAL, SCOPE,
FORBIDDEN, REQUIREMENTS, DEFINITION OF DONE. Units are numbered starting at
U90 to avoid collision with the Linux implementation (which ended at ~U85).

---

## Milestone 1: Basic Platform Adapter (U90-U92)

### U90: macOS Self-Process Metrics

**GOAL**: Implement `MacosPlatformAdapter::self_process_metrics()` so the
service's `/health` endpoint reports real RSS and CPU time on macOS.

**SCOPE** (may create/modify):
- `rust_core/platform/src/macos/mod.rs` — replace the `self_process_metrics`
  stub with a real implementation using `getrusage(RUSAGE_SELF)`
- Add any necessary `libc` imports for macOS-specific constants
- `rust_core/platform/src/macos/mod.rs` tests — add a unit test
  `self_process_metrics_returns_real_values` that calls the method and asserts
  RSS > 0 and CPU time > 0

**FORBIDDEN**:
- Do NOT touch `platform/src/linux/` — Linux's implementation stays unchanged
- Do NOT implement any other `PlatformAdapter` methods yet
- Do NOT create new modules in `platform/src/macos/` yet

**REQUIREMENTS**:
1. Use `libc::getrusage(libc::RUSAGE_SELF, ...)` — same POSIX call Linux uses
2. macOS reports `ru_maxrss` in **bytes**, not kilobytes like Linux — no
   multiplication by 1024 needed
3. Sum user time (`ru_utime`) and system time (`ru_stime`) for total CPU time
4. Return a real `ProcessSelfMetrics { rss_bytes, cpu_time }`
5. If `getrusage` fails, return `CapabilityError::Io(std::io::Error::last_os_error())`
6. Use `#[allow(unsafe_code)]` on the function that contains the unsafe block

**DEFINITION OF DONE**:
- [ ] `cargo check --target aarch64-apple-darwin` succeeds
- [ ] `cargo test --target aarch64-apple-darwin -p platform` passes the new test
- [ ] Running `curl --unix-socket $XDG_RUNTIME_DIR/... /api/v1/health` on a
      real macOS machine returns non-zero RSS/CPU values (manual verification)
- [ ] No new clippy warnings
- [ ] `self_process_metrics` method no longer returns `Unsupported`

**Vibe Coding Prompt**:
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

**GOAL**: Implement process priority getting/setting on macOS using
`setpriority(2)`, enabling the same priority actions Linux already has.

**SCOPE** (may create/modify):
- `rust_core/platform/src/macos/mod.rs` — implement `get_process_priority` and
  `set_process_priority`
- Create `rust_core/platform/src/macos/process_control.rs` following the same
  pattern as `platform/src/linux/process_control.rs`
- Add unit tests in `process_control.rs`
- Update `mod.rs` to `pub mod process_control;` and call through to it

**FORBIDDEN**:
- Do NOT implement CPU affinity yet (that's U92)
- Do NOT implement suspend/resume yet (that's U93)
- Do NOT modify `core/src/actions/host/` — it already calls through the trait

**REQUIREMENTS**:
1. Use `libc::setpriority(libc::PRIO_PROCESS, pid as i32, nice_value)`
2. Map `ProcessPriority` enum to nice values:
   - `Idle` → 20
   - `BelowNormal` → 10
   - `Normal` → 0
   - `AboveNormal` → -10
   - `High` → -20
3. Use `libc::getpriority(libc::PRIO_PROCESS, pid as i32)` to read current priority
4. Handle `getpriority` returning -1 by checking `errno` (real error vs. valid -1)
5. Raising priority (negative nice) without privilege returns
   `CapabilityError::PermissionRequired` with message about lacking privilege
6. Unit tests: `get_priority_of_self_is_a_real_value`,
   `lowering_priority_succeeds_unprivileged`,
   `raising_priority_fails_unprivileged`

**DEFINITION OF DONE**:
- [ ] Both methods compile and pass tests on `aarch64-apple-darwin`
- [ ] `core/tests/actions_host_priority_affinity_test.rs` compiles on macOS
      (may skip affinity tests, but priority tests should work)
- [ ] Manually verified on real macOS: lowering priority succeeds, raising
      priority fails with `PermissionRequired` when unprivileged
- [ ] CI's `check-macos` job still passes
- [ ] No changes to `core/src/actions/host/priority.rs` were needed

**Vibe Coding Prompt**:
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

### U92: macOS CPU Affinity (Design Decision Required)

**GOAL**: Design and document the macOS CPU affinity approach, acknowledging
that macOS has no `sched_setaffinity` equivalent.

**SCOPE** (may create/modify):
- Create `docs/adr/0086-macos-cpu-affinity-approach.md`
- Update `rust_core/platform/src/macos/mod.rs` with the chosen approach OR
  document why CPU affinity remains `Unsupported` on macOS
- If implementing: add to `process_control.rs`

**FORBIDDEN**:
- Do NOT silently stub it as `Unsupported` without an ADR explaining why
- Do NOT use Linux's `sched_setaffinity` approach (doesn't exist on macOS)

**REQUIREMENTS**:
1. Research macOS options:
   - `thread_policy_set` with `THREAD_AFFINITY_POLICY` (per-thread hint, not process-level mask)
   - Quality of Service (QoS) classes (different abstraction level)
   - Accept that macOS doesn't have hard affinity masks
2. Choose one of:
   - **Option A**: Return `CapabilityError::Unsupported` with explanation that
     macOS lacks hard affinity masks — document in ADR 0086
   - **Option B**: Implement a best-effort `thread_policy_set` approach that
     approximates affinity via thread affinity hints — document limitations
   - **Option C**: Map to QoS classes instead of CPU indices — document the
     semantic difference
3. If choosing Option A, update `core/tests/actions_host_priority_affinity_test.rs`
   to `#[cfg(target_os = "linux")]` gate the affinity tests
4. Document the decision in an ADR with rationale

**DEFINITION OF DONE**:
- [ ] `docs/adr/0086-macos-cpu-affinity-approach.md` exists and explains the decision
- [ ] Either a real implementation exists OR `Unsupported` is returned with a
      clear reason referencing ADR 0086
- [ ] Tests compile on macOS (skipped if Unsupported, or test the implemented approach)
- [ ] HANDOFF.md's §6 note about `sched_setaffinity` is either resolved or
      expanded with the ADR reference

**Vibe Coding Prompt**:
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

## Milestone 2: Process Control (U93-U94)

### U93: macOS Process Suspend/Resume

**GOAL**: Implement `suspend_process`, `resume_process`, and `is_process_stopped`
using SIGSTOP/SIGCONT, mirroring Linux's approach.

**SCOPE** (may create/modify):
- `rust_core/platform/src/macos/process_control.rs` — add suspend/resume/is_stopped
- Add unit tests
- Update `mod.rs` to call these methods

**FORBIDDEN**:
- Do NOT use per-thread suspend (Windows-style) — macOS has SIGSTOP/SIGCONT
- Do NOT modify `core/src/actions/host/suspend.rs` — it's already OS-agnostic

**REQUIREMENTS**:
1. `suspend(pid)`: `libc::kill(pid as i32, libc::SIGSTOP)`
2. `resume(pid)`: `libc::kill(pid as i32, libc::SIGCONT)`
3. `is_stopped(pid)`: Read `/proc/[pid]/stat` is Linux-specific — on macOS, use
   `proc_pidinfo(pid, PROC_PIDTASKINFO, ...)` and check `pti_flags & P_TRACED`
   or `pti_status` for stopped state
4. Alternative: Parse `ps -o state= -p [pid]` output for "T" state (simpler, less efficient)
5. Handle ESRCH (no such process) as `CapabilityError::NotFound`
6. Handle EPERM as `CapabilityError::PermissionRequired`

**DEFINITION OF DONE**:
- [ ] All three methods implemented and compile on macOS
- [ ] Unit test: spawn a child process, suspend it, verify `is_stopped` returns true,
      resume it, verify `is_stopped` returns false
- [ ] Manually verified on real macOS with a real process
- [ ] `core/tests/actions_host_suspend_test.rs` passes on macOS (if it exists)

**Vibe Coding Prompt**:
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

**GOAL**: Create the `exec.rs` module for macOS, establishing the allowed
`std::process::Command` execution surface.

**SCOPE** (may create/modify):
- `rust_core/platform/src/macos/exec.rs` — currently empty, add module structure
- Add unit test that verifies `/bin/echo hello` works

**FORBIDDEN**:
- Do NOT implement any exec-based actions yet (those come later with telemetry)
- Do NOT add `Command` calls anywhere outside this module

**REQUIREMENTS**:
1. Create a basic structure mirroring `platform/src/linux/exec.rs`
2. Add a wrapper function pattern that will be used by future telemetry
3. Document that this is the ONLY module allowed to call `std::process::Command`
   in the macOS platform crate (enforced by CI grep gate)
4. Add basic test: execute `/bin/echo` and verify output

**DEFINITION OF DONE**:
- [ ] `rust_core/platform/src/macos/exec.rs` exists and compiles
- [ ] Contains a test that executes a real command
- [ ] `scripts/check-no-command-outside-exec.sh` still passes (exec.rs is allowed)
- [ ] Module is `pub mod exec;` in `mod.rs`

**Vibe Coding Prompt**:
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

## Milestone 3: Host Telemetry Foundation (U95-U100)

### U95: macOS CPU Telemetry via host_statistics64

**GOAL**: Implement system-wide CPU utilization snapshot on macOS using
`host_statistics64` Mach API.

**SCOPE** (may create/modify):
- Create `rust_core/core/src/telemetry_macos/` (new module, gated `#[cfg(target_os = "macos")]`)
- Create `rust_core/core/src/telemetry_macos/cpu.rs`
- Add to `rust_core/core/src/lib.rs`: `#[cfg(target_os = "macos")] pub mod telemetry_macos;`
- Mirror `core/src/telemetry/cpu.rs` structure but use macOS APIs

**FORBIDDEN**:
- Do NOT modify `core/src/telemetry/` (Linux-only, stays separate)
- Do NOT try to share code between Linux and macOS telemetry yet — establish
  the macOS version first, refactor later if beneficial

**REQUIREMENTS**:
1. Use `host_statistics64(mach_host_self(), HOST_CPU_LOAD_INFO, ...)` to get
   CPU ticks across all cores
2. Define FFI bindings for `host_cpu_load_info_data_t` with fields:
   - `cpu_ticks[CPU_STATE_MAX]` where states are USER, SYSTEM, IDLE, NICE
3. Calculate total CPU usage: `(user + system + nice) / (user + system + idle + nice) * 100.0`
4. For per-core utilization, use `host_processor_info` with `PROCESSOR_CPU_LOAD_INFO`
5. Return a structure mirroring `contracts::CpuSnapshot` (reuse the wire type)
6. Handle sampling intervals: first sample returns 0% (no delta), subsequent
   samples compute utilization from tick deltas

**DEFINITION OF DONE**:
- [ ] Can call the telemetry function and get a CpuSnapshot on macOS
- [ ] Unit test: two samples 100ms apart, verify utilization is >= 0.0 and <= 100.0
- [ ] Compiles on `aarch64-apple-darwin`
- [ ] CI's `check-macos` passes

**Vibe Coding Prompt**:
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

**GOAL**: Implement memory snapshot on macOS using `vm_statistics64`.

**SCOPE** (may create/modify):
- `rust_core/core/src/telemetry_macos/memory.rs`
- FFI bindings for `vm_statistics64_data_t`

**FORBIDDEN**:
- Do NOT try to read cgroup memory (macOS doesn't have cgroups)
- Do NOT implement swap accounting yet (separate unit)

**REQUIREMENTS**:
1. Use `host_statistics64(mach_host_self(), HOST_VM_INFO64, ...)` for memory stats
2. Parse `vm_statistics64_data_t` fields:
   - `free_count`, `active_count`, `inactive_count`, `wire_count`, `speculative_count`
   - Multiply by `PAGE_SIZE` to get bytes
3. Calculate pressure tiers:
   - **None**: free + inactive > 20% of total
   - **Low**: free + inactive 10-20%
   - **Medium**: free + inactive 5-10%
   - **High**: free + inactive < 5%
4. Get total physical memory via `sysctl hw.memsize`
5. Return `contracts::MemorySnapshot`

**DEFINITION OF DONE**:
- [ ] `memory_snapshot()` returns real macOS memory data
- [ ] Unit test verifies total_bytes > 0, available_bytes > 0
- [ ] Pressure tier is one of the four valid values
- [ ] Compiles and tests pass on macOS

**Vibe Coding Prompt**:
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

**GOAL**: Implement storage/filesystem telemetry on macOS.

**SCOPE** (may create/modify):
- `rust_core/core/src/telemetry_macos/storage.rs`
- Use `statfs` (POSIX, same as Linux)

**FORBIDDEN**:
- Do NOT implement disk I/O stats yet (that's U98)
- Do NOT use IOKit for this unit (statfs is sufficient for capacity)

**REQUIREMENTS**:
1. Use `libc::statfs(path, buf)` for each filesystem
2. Enumerate mount points via `getfsstat` or parse `/etc/fstab` equivalent
3. Calculate capacity/used/available from `f_blocks`, `f_bfree`, `f_bavail`
4. Skip virtual filesystems (devfs, autofs, etc.) like Linux does
5. Calculate pressure tiers based on used percentage (>90% = High, etc.)
6. Return `contracts::StorageSnapshot`

**DEFINITION OF DONE**:
- [ ] `storage_snapshot()` returns real filesystem data
- [ ] Lists at least the root filesystem (/)
- [ ] Capacity, used, available are correct (verified manually with `df`)
- [ ] Unit test verifies at least one filesystem returned

**Vibe Coding Prompt**:
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

**GOAL**: Implement disk I/O statistics on macOS using IOKit.

**SCOPE** (may create/modify):
- `rust_core/core/src/telemetry_macos/storage.rs` — extend with I/O stats
- Add IOKit FFI bindings or use `io-kit-sys` crate

**FORBIDDEN**:
- Do NOT implement network I/O yet (that's U99)

**REQUIREMENTS**:
1. Use IOKit's `IOServiceMatching("IOMedia")` to enumerate disks
2. Get `IOBlockStorageDriver` statistics:
   - `kIOBlockStorageDriverStatisticsBytesReadKey`
   - `kIOBlockStorageDriverStatisticsBytesWrittenKey`
   - `kIOBlockStorageDriverStatisticsReadsKey`
   - `kIOBlockStorageDriverStatisticsWritesKey`
3. Calculate read/write throughput from byte deltas over time
4. Add to `StorageSnapshot` or create separate `DiskIoSnapshot`
5. Alternative: use `iostat` command parsing via exec.rs if IOKit is too complex

**DEFINITION OF DONE**:
- [ ] Can read disk I/O stats on macOS
- [ ] Returns bytes read/written per disk
- [ ] Unit test verifies non-negative values
- [ ] Decision documented: IOKit vs iostat parsing

**Vibe Coding Prompt**:
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

**GOAL**: Implement network interface statistics on macOS.

**SCOPE** (may create/modify):
- `rust_core/core/src/telemetry_macos/network.rs`
- Parse `netstat -ibn` or use `sysctl` for network stats

**FORBIDDEN**:
- Do NOT implement per-process network I/O (Linux doesn't have this either)

**REQUIREMENTS**:
1. Use `sysctl` with `net.link.generic.system` or parse `netstat -ibn`
2. Get per-interface stats:
   - bytes in/out
   - packets in/out
   - errors
3. Filter out loopback (lo0) unless explicitly requested
4. Calculate throughput from byte deltas
5. Return `contracts::NetworkSnapshot`

**DEFINITION OF DONE**:
- [ ] `network_snapshot()` returns real interface data
- [ ] Lists active interfaces (at minimum en0 on macOS)
- [ ] Bytes in/out are correct (compare with `netstat -ibn` manually)
- [ ] Unit test verifies at least one interface

**Vibe Coding Prompt**:
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

**GOAL**: Implement process listing on macOS using `libproc`.

**SCOPE** (may create/modify):
- `rust_core/core/src/telemetry_macos/process.rs`
- FFI bindings for `libproc` or use `libproc` crate

**FORBIDDEN**:
- Do NOT implement per-process network I/O (gap on both platforms)
- Do NOT implement process control here (already done in platform crate)

**REQUIREMENTS**:
1. Use `proc_listallpids` to enumerate all process IDs
2. For each PID, use `proc_pidinfo(PROC_PIDTASKINFO)` to get:
   - Virtual memory size (pti_virtual_size)
   - Resident memory (pti_resident_size)
   - CPU time (pti_total_user + pti_total_system)
   - Process state
3. Use `proc_pidpath` to get executable path
4. For command line args, use `sysctl KERN_PROCARGS2`
5. Background vs foreground: check if process has controlling terminal
6. Return `contracts::ProcessSnapshot` with Vec<ProcessInfo>

**DEFINITION OF DONE**:
- [ ] `process_snapshot()` returns list of running processes
- [ ] Each process has PID, name, memory, CPU usage
- [ ] Can identify at least the calling process itself
- [ ] Unit test verifies current process appears in list

**Vibe Coding Prompt**:
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

## Milestone 4: Integration & Testing (U101-U105)

### U101: Wire macOS Telemetry into Service

**GOAL**: Expose all macOS telemetry through the HTTP API.

**SCOPE** (may create/modify):
- `rust_core/service/src/telemetry/sampler.rs` — add macOS conditional compilation
- `rust_core/service/src/routes/telemetry.rs` — may need minor changes for macOS
- `rust_core/core/src/lib.rs` — ensure `telemetry_macos` is public

**FORBIDDEN**:
- Do NOT break Linux telemetry (must support both)
- Do NOT duplicate route definitions

**REQUIREMENTS**:
1. In `sampler.rs`, wrap Linux telemetry in `#[cfg(target_os = "linux")]`
2. Add `#[cfg(target_os = "macos")]` section that calls `telemetry_macos::`
   functions instead
3. Both compile to the same `TelemetrySnapshot` types (contracts are shared)
4. All existing routes (`/api/v1/cpu`, `/memory`, etc.) work on macOS now
5. Test on real macOS: `curl --unix-socket ... /api/v1/cpu` returns real data

**DEFINITION OF DONE**:
- [ ] Service builds on both Linux and macOS
- [ ] `/api/v1/cpu` returns real macOS data when running on macOS
- [ ] All telemetry endpoints work: memory, storage, network, processes
- [ ] CI's `check-macos` job passes
- [ ] Manually verified on real macOS machine

**Vibe Coding Prompt**:
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

**GOAL**: Run the full AsterOpsAI test suite on macOS and fix any failures.

**SCOPE** (may create/modify):
- Any test file that has platform-specific assumptions
- Add `#[cfg(target_os = "linux")]` gates where needed
- Fix any macOS-specific test failures

**FORBIDDEN**:
- Do NOT skip tests without understanding why they fail
- Do NOT change core business logic to make tests pass (fix the tests instead)

**REQUIREMENTS**:
1. Run `cargo test --workspace` on macOS
2. For each failure:
   - Understand root cause
   - If Linux-specific (e.g., `/proc` parsing), gate with `#[cfg(target_os = "linux")]`
   - If genuinely cross-platform, fix the implementation
3. Document any test gaps (tests that should exist on macOS but don't yet)
4. All `contracts`, `core` (non-telemetry), `repository`, `dbms`, `policy`,
   `actions`, `ai`, `security` tests should pass on macOS unchanged

**DEFINITION OF DONE**:
- [ ] `cargo test --workspace` passes on macOS
- [ ] Any skipped tests are documented with reasons
- [ ] No test modifications that break Linux tests
- [ ] CI can run macOS tests (may need GitHub Actions macOS runner)

**Vibe Coding Prompt**:
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

**GOAL**: Build and run the Flutter console on macOS desktop.

**SCOPE** (may create/modify):
- `console/` directory — may need macOS-specific adjustments
- `console/macos/` directory — Flutter's macOS target files

**FORBIDDEN**:
- Do NOT create a separate macOS console implementation (same codebase)
- Do NOT modify API contracts (console is thin, remember)

**REQUIREMENTS**:
1. Install Flutter macOS desktop support: `flutter config --enable-macos-desktop`
2. Run `flutter build macos` in console directory
3. Verify Unix socket path works on macOS (should be identical)
4. Test all screens: dashboard, CPU, memory, storage, database, policy, security
5. Verify l10n works (internationalization)
6. Check fonts, window chrome, native controls

**DEFINITION OF DONE**:
- [ ] `flutter build macos` succeeds
- [ ] `flutter run -d macos` launches the console
- [ ] Can connect to running service on macOS
- [ ] All screens render correctly
- [ ] No platform-specific crashes
- [ ] Console tests pass: `flutter test` (if any exist)

**Vibe Coding Prompt**:
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

**GOAL**: Adapt the Linux demo scenarios for macOS or create macOS equivalents.

**SCOPE** (may create/modify):
- `scripts/demo/` — may need macOS versions
- New `docs/demo/RUNBOOK-MACOS.md`

**FORBIDDEN**:
- Do NOT require cgroup v2 (macOS doesn't have cgroups)
- Do NOT require Linux-specific tools

**REQUIREMENTS**:
1. Review existing demo scenarios:
   - Lock storm (should work identically — PostgreSQL behavior)
   - Pool exhaustion (should work identically)
   - Storage latency (needs different approach without cgroups)
2. For storage latency on macOS:
   - Use `sudo pfctl` for rate limiting? (complex)
   - Or use a slow external drive / network mount
   - Or use `dd` with throttling
3. Create macOS runbook documenting how to execute demos
4. Verify correlation engine produces same results on macOS

**DEFINITION OF DONE**:
- [ ] At least 2 of 3 demo scenarios work on macOS
- [ ] `docs/demo/RUNBOOK-MACOS.md` exists with instructions
- [ ] Correlation engine correctly identifies causes on macOS
- [ ] Screenshots/output samples included in docs

**Vibe Coding Prompt**:
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

**GOAL**: Add macOS build/test to GitHub Actions CI.

**SCOPE** (may create/modify):
- `.github/workflows/ci.yml` — add macOS jobs
- May need separate workflow file for macOS

**FORBIDDEN**:
- Do NOT break existing Linux CI
- Do NOT make macOS CI required until it's stable

**REQUIREMENTS**:
1. Add `check-macos-build` job that runs `cargo build --workspace` on `macos-latest`
2. Add `test-macos` job that runs `cargo test --workspace` on `macos-latest`
3. Add `console-macos` job that runs `flutter build macos && flutter test`
4. Use `actions/cache` for Cargo and Flutter dependencies
5. Initially make macOS jobs non-blocking (allow failure while stabilizing)
6. Once stable, add to required checks

**DEFINITION OF DONE**:
- [ ] CI runs on macOS runner
- [ ] Build and test jobs complete (pass or documented failures)
- [ ] CI time is reasonable (<10 minutes for macOS)
- [ ] Documented plan for making macOS jobs required

**Vibe Coding Prompt**:
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

## Milestone 5: Documentation & Polish (U106-U110)

### U106: Update ARCHITECTURE.md with macOS Implementation

**GOAL**: Document the macOS platform adapter implementation in architecture docs.

**SCOPE** (may create/modify):
- Create or update `docs/ARCHITECTURE.md`
- Update `HANDOFF.md` §7 to reflect completed macOS work
- Update `README.md` to remove "Linux only" caveats

**FORBIDDEN**:
- Do NOT document unimplemented features as complete

**REQUIREMENTS**:
1. Add macOS section to architecture docs
2. Document which telemetry metrics are supported
3. Note any gaps (e.g., CPU affinity if unsupported)
4. Update platform adapter diagram if one exists
5. Cross-reference relevant ADRs (especially 0086 for CPU affinity)

**DEFINITION OF DONE**:
- [ ] ARCHITECTURE.md (or new doc) describes macOS implementation
- [ ] HANDOFF.md updated to reflect work done
- [ ] README.md updated to mention macOS support
- [ ] All docs reviewed for stale "Linux only" references

**Vibe Coding Prompt**:
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

**GOAL**: Benchmark telemetry performance on macOS and compare to Linux.

**SCOPE** (may create/modify):
- Create `benches/macos_telemetry_bench.rs` using Criterion
- Document performance characteristics

**FORBIDDEN**:
- Do NOT optimize prematurely (measure first)

**REQUIREMENTS**:
1. Benchmark each telemetry function:
   - CPU snapshot time
   - Memory snapshot time
   - Process enumeration time
   - Full telemetry cycle time
2. Compare to Linux benchmarks (if they exist)
3. Identify any performance outliers (>100ms for any single metric)
4. Document acceptable performance thresholds
5. Add to CI as non-blocking performance regression detection

**DEFINITION OF DONE**:
- [ ] Benchmarks exist and run on macOS
- [ ] Performance documented in docs/performance-macos.md
- [ ] Any performance issues identified and ticketed
- [ ] CI runs benchmarks (informational, not blocking)

**Vibe Coding Prompt**:
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

**GOAL**: Write user-facing installation instructions for macOS.

**SCOPE** (may create/modify):
- Create `docs/INSTALL-MACOS.md`
- Update main README.md with macOS quickstart

**FORBIDDEN**:
- Do NOT write deployment guides for production yet (that's v2)

**REQUIREMENTS**:
1. Prerequisites:
   - Xcode Command Line Tools
   - Rust toolchain
   - Flutter (if running console)
   - PostgreSQL (for monitoring)
2. Build instructions: `cargo build --release`
3. Running the service: `./target/release/service`
4. Socket location: `$HOME/Library/Application Support/ai-ops-coordinator/core.sock`
   (or macOS equivalent of XDG_RUNTIME_DIR)
5. Security: explain Unix socket permissions
6. Troubleshooting section

**DEFINITION OF DONE**:
- [ ] INSTALL-MACOS.md exists and is tested
- [ ] Fresh macOS machine can follow instructions and get running
- [ ] Common issues documented
- [ ] README.md links to it

**Vibe Coding Prompt**:
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

**GOAL**: Document macOS-specific security considerations and TCC permissions.

**SCOPE** (may create/modify):
- Create `docs/security-macos.md`
- Document TCC prompt scenarios
- Create user guide for granting permissions

**FORBIDDEN**:
- Do NOT request unnecessary permissions
- Do NOT implement privileged helper yet (future work)

**REQUIREMENTS**:
1. Document which macOS permissions AsterOpsAI needs:
   - Full Disk Access (for reading other processes' info)? — verify if needed
   - Automation (for GUI scripting)? — probably not
   - Input Monitoring? — definitely not
2. Show how permissions appear in System Settings
3. Explain Unix socket security model on macOS
4. Document what works without elevated privileges
5. Future: note where SMAppService helper would be needed (reference TRS §39)

**DEFINITION OF DONE**:
- [ ] Security doc exists
- [ ] Tested permission prompts on real macOS
- [ ] User knows how to grant necessary permissions
- [ ] Documented what degrades gracefully without permissions

**Vibe Coding Prompt**:
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

**GOAL**: Create release process for first macOS-supported version.

**SCOPE** (may create/modify):
- Create `docs/RELEASE-CHECKLIST.md`
- Tag version (e.g., v0.2.0-macos-preview)
- Create GitHub release notes

**FORBIDDEN**:
- Do NOT claim production-ready yet (this is preview/beta)
- Do NOT skip testing steps

**REQUIREMENTS**:
1. Pre-release checklist:
   - [ ] All macOS unit tests pass
   - [ ] All macOS integration tests pass
   - [ ] Console builds and runs on macOS
   - [ ] Demo scenarios work on macOS
   - [ ] Documentation is updated
   - [ ] Performance is acceptable
   - [ ] Security is documented
2. Version tagging: follow semver (0.2.0 = minor version, new platform)
3. Release notes highlighting:
   - macOS support added
   - What works (full telemetry, actions, console)
   - Known limitations (any unsupported features)
   - Installation instructions link
4. Create binary releases for macOS (optional, or wait for v1.0)

**DEFINITION OF DONE**:
- [ ] Release checklist document exists
- [ ] All checklist items completed
- [ ] Git tag created and pushed
- [ ] GitHub release created with notes
- [ ] Announced in appropriate channels

**Vibe Coding Prompt**:
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

## Summary: macOS Development Roadmap

### Milestones Overview

| Milestone | Units | Focus Area | Estimated Complexity |
|-----------|-------|------------|---------------------|
| M1: Basic Platform Adapter | U90-U92 | Process metrics, priority, affinity | Low |
| M2: Process Control | U93-U94 | Suspend/resume, exec foundation | Low |
| M3: Host Telemetry | U95-U100 | CPU, memory, storage, network, processes | High |
| M4: Integration & Testing | U101-U105 | Wire to service, test, CI | Medium |
| M5: Documentation & Polish | U106-U110 | Docs, benchmarks, release | Low-Medium |

### Critical Path Items

1. **U90** (self-process metrics) — Unblocks service health endpoint
2. **U95-U100** (telemetry) — Longest/hardest work, parallelizable
3. **U101** (integration) — Unblocks end-to-end testing
4. **U102** (test suite) — Quality gate before release

### Parallel Work Opportunities

- U95-U100 can be done in parallel by different developers (each telemetry module is independent)
- U103 (console) can be done alongside U95-U100 (doesn't depend on telemetry)
- U107 (benchmarks) and U106 (docs) can be done in parallel with U105 (CI)

### Estimated Timeline

- **Week 1-2**: M1 + M2 (Basic platform adapter) — 4 units
- **Week 3-5**: M3 (Host telemetry) — 6 units, biggest lift
- **Week 6**: M4 (Integration) — 5 units
- **Week 7**: M5 (Polish) — 5 units
- **Total**: ~7-8 weeks for complete macOS parity with Linux

### Success Criteria

At the end of U110, you should have:
- ✅ Full macOS platform adapter implementation
- ✅ Complete telemetry for CPU, memory, storage, network, processes
- ✅ All tests passing on macOS
- ✅ Console running on macOS desktop
- ✅ Demo scenarios working on macOS
- ✅ CI building and testing on macOS
- ✅ Documentation complete
- ✅ First macOS release tagged

---

## Next Steps

1. **Review this plan** — validate that priorities align with your goals
2. **Set up macOS dev environment** — Rust, Flutter, PostgreSQL for testing
3. **Start with U90** — quick win, unblocks health endpoint testing
4. **Decide on CPU affinity (U92)** — this is the only design decision before coding
5. **Parallelize U95-U100** if you have multiple contributors

Each unit includes a "Vibe Coding Prompt" ready to paste into Claude Code
for implementation assistance. The prompts are context-rich and reference
existing code patterns to maintain consistency with the Linux implementation.

Good luck with the macOS port! 🚀
