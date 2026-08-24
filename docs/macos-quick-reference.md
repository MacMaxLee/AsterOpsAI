# macOS Development Quick Reference

Fast lookup for common commands, APIs, and patterns during macOS development.

---

## Build & Test Commands

### Rust Core
```bash
# Check compilation
cargo check --target aarch64-apple-darwin

# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Test specific package
cargo test -p platform
cargo test -p core

# Run with specific target
cargo test --target aarch64-apple-darwin

# Run benchmarks
cargo bench

# Clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Format
cargo fmt --all -- --check
```

### Flutter Console
```bash
# Enable macOS desktop support
flutter config --enable-macos-desktop

# Build for macOS
cd console
flutter build macos

# Run on macOS
flutter run -d macos

# Test
flutter test

# Analyze
flutter analyze
```

### Demo Scenarios
```bash
# Requires PostgreSQL running
cd scripts/demo

# Lock storm (should work as-is on macOS)
./lock-storm.sh

# Pool exhaustion (should work as-is)
./pool-exhaustion.sh

# Storage latency (needs macOS adaptation)
# See docs/demo/RUNBOOK-MACOS.md
```

---

## macOS System APIs Quick Reference

### Process Information (libproc)

```rust
use libc::{proc_listallpids, proc_pidinfo, proc_pidpath};

// List all process IDs
let mut pids = vec![0i32; 1024];
let count = unsafe { proc_listallpids(pids.as_mut_ptr() as *mut _, 1024) };

// Get process info
let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
let size = std::mem::size_of::<libc::proc_taskinfo>() as i32;
let ret = unsafe {
    proc_pidinfo(
        pid,
        libc::PROC_PIDTASKINFO,
        0,
        &mut info as *mut _ as *mut _,
        size
    )
};

// Get process path
let mut path = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
let ret = unsafe {
    proc_pidpath(pid, path.as_mut_ptr() as *mut _, path.len() as u32)
};
```

### CPU Statistics (Mach)

```rust
// System-wide CPU
extern "C" {
    fn host_statistics64(
        host: libc::host_t,
        flavor: libc::host_flavor_t,
        host_info: *mut libc::integer_t,
        count: *mut libc::mach_msg_type_number_t,
    ) -> libc::kern_return_t;
}

// Per-core CPU
extern "C" {
    fn host_processor_info(
        host: libc::host_t,
        flavor: libc::processor_flavor_t,
        out_processor_count: *mut libc::natural_t,
        out_processor_info: *mut *mut libc::integer_t,
        out_processor_info_count: *mut libc::mach_msg_type_number_t,
    ) -> libc::kern_return_t;
}

// Get mach host
let host = unsafe { libc::mach_host_self() };
```

### Memory Statistics (Mach)

```rust
// VM statistics
// Use host_statistics64 with HOST_VM_INFO64
const HOST_VM_INFO64: i32 = 4;

// vm_statistics64_data_t fields:
// - free_count
// - active_count
// - inactive_count
// - wire_count
// - zero_fill_count
// - reactivations
// - pageins
// - pageouts
// - faults
// - cow_faults
// - lookups
// - hits
// - purges
// - purgeable_count
// - speculative_count

// Get total RAM
use std::mem;
let mut size: u64 = 0;
let mut len = mem::size_of::<u64>();
unsafe {
    libc::sysctlbyname(
        b"hw.memsize\0".as_ptr() as *const _,
        &mut size as *mut _ as *mut _,
        &mut len,
        std::ptr::null_mut(),
        0,
    );
}
```

### Storage (POSIX)

```rust
use libc::statfs;

// Get filesystem stats (same as Linux)
let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
let path = std::ffi::CString::new("/").unwrap();
let ret = unsafe { libc::statfs(path.as_ptr(), &mut stat) };

// Enumerate filesystems
use libc::getfsstat;
let mut bufsize = unsafe { getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
let mut mounts = vec![unsafe { std::mem::zeroed() }; bufsize as usize];
bufsize = unsafe { getfsstat(mounts.as_mut_ptr(), bufsize, libc::MNT_NOWAIT) };
```

### Network (sysctl or netstat)

```rust
// Option 1: sysctl approach
// Use sysctl with net.link.generic.system

// Option 2: Parse netstat (simpler for v1)
use std::process::Command;
let output = Command::new("netstat")
    .arg("-ibn")
    .output()
    .expect("netstat failed");
// Parse output.stdout
```

### Process Priority

```rust
use libc::{getpriority, setpriority, PRIO_PROCESS};

// Get priority
let nice = unsafe { getpriority(PRIO_PROCESS, pid as u32) };
// Note: getpriority can return -1 as valid value, check errno

// Set priority
let ret = unsafe { setpriority(PRIO_PROCESS, pid as u32, nice_value) };
```

### Process Suspend/Resume

```rust
use libc::{kill, SIGSTOP, SIGCONT};

// Suspend
let ret = unsafe { kill(pid as i32, SIGSTOP) };

// Resume
let ret = unsafe { kill(pid as i32, SIGCONT) };

// Check if stopped (parse ps output)
use std::process::Command;
let output = Command::new("ps")
    .arg("-o").arg("state=")
    .arg("-p").arg(pid.to_string())
    .output()
    .expect("ps failed");
let state = String::from_utf8_lossy(&output.stdout);
let is_stopped = state.trim() == "T";
```

### Self-Process Metrics

```rust
use libc::{getrusage, rusage, RUSAGE_SELF};

// Get resource usage
let mut usage: rusage = unsafe { std::mem::zeroed() };
let ret = unsafe { getrusage(RUSAGE_SELF, &mut usage) };

// Extract metrics
let cpu_time = Duration::from_secs(
    usage.ru_utime.tv_sec as u64 + usage.ru_stime.tv_sec as u64
) + Duration::from_micros(
    usage.ru_utime.tv_usec as u64 + usage.ru_stime.tv_usec as u64
);

// macOS reports ru_maxrss in BYTES (Linux uses kilobytes)
let rss_bytes = usage.ru_maxrss as u64;
```

---

## Common File Paths on macOS

### Unix Socket Location
```bash
# XDG_RUNTIME_DIR doesn't exist on macOS by default
# Use Application Support instead:
~/Library/Application Support/ai-ops-coordinator/core.sock

# Or use temporary directory:
/tmp/ai-ops-coordinator-$USER/core.sock
```

### Configuration
```bash
# User config
~/Library/Application Support/ai-ops-coordinator/config.toml

# System config (if needed)
/Library/Application Support/ai-ops-coordinator/config.toml
```

### Logs
```bash
~/Library/Logs/ai-ops-coordinator/
```

### Database
```bash
~/Library/Application Support/ai-ops-coordinator/state.db
```

---

## FFI Patterns

### Safe Wrapper Template
```rust
#[allow(unsafe_code)]
fn safe_wrapper(pid: u32) -> Result<SomeType, CapabilityError> {
    let mut data = std::mem::MaybeUninit::<libc::some_struct>::zeroed();

    // SAFETY: data is a valid, correctly-sized, writable buffer for the
    // duration of this call; syscall only ever writes to it and returns
    // a status code we check before treating it as initialized.
    let rc = unsafe { libc::some_syscall(pid as i32, data.as_mut_ptr()) };

    if rc != 0 {
        return Err(CapabilityError::Io(std::io::Error::last_os_error()));
    }

    // SAFETY: rc == 0 guarantees the kernel fully populated data.
    let result = unsafe { data.assume_init() };

    Ok(convert_to_rust_type(result))
}
```

### Error Handling
```rust
use std::io;

// Check errno for system calls that use it
let err = io::Error::last_os_error();
match err.raw_os_error() {
    Some(libc::ESRCH) => Err(CapabilityError::NotFound("process not found")),
    Some(libc::EPERM) => Err(CapabilityError::PermissionRequired("...")),
    Some(libc::EINVAL) => Err(CapabilityError::InvalidInput("...")),
    _ => Err(CapabilityError::Io(err)),
}
```

---

## Debugging Techniques

### Check if Syscall Exists
```bash
# Check for function in libc
nm /usr/lib/libSystem.B.dylib | grep function_name

# Example: check for proc_pidinfo
nm /usr/lib/libSystem.B.dylib | grep proc_pidinfo
```

### Test API Behavior
```rust
// Quick test in a separate binary
fn main() {
    let pid = std::process::id();
    println!("Testing with PID: {}", pid);

    // Test your API here
    let result = get_process_priority(pid);
    println!("Result: {:?}", result);
}
```

### Verify Against System Tools
```bash
# CPU
top -l 1 | grep "CPU usage"

# Memory
vm_stat

# Storage
df -h

# Network
netstat -ibn

# Processes
ps aux

# Process priority
ps -o pid,nice,comm

# Process state
ps -o pid,state,comm
```

---

## Conditional Compilation Patterns

### Target OS Gating
```rust
#[cfg(target_os = "linux")]
use crate::telemetry;

#[cfg(target_os = "macos")]
use crate::telemetry_macos as telemetry;

// Or in function bodies:
#[cfg(target_os = "linux")]
let result = linux_specific_function();

#[cfg(target_os = "macos")]
let result = macos_specific_function();
```

### Feature Gating (if needed)
```rust
#[cfg(all(target_os = "macos", feature = "iokit"))]
use iokit_based_implementation;

#[cfg(all(target_os = "macos", not(feature = "iokit")))]
use fallback_implementation;
```

### Test Gating
```rust
#[test]
#[cfg(target_os = "macos")]
fn macos_specific_test() {
    // ...
}

#[test]
#[cfg(unix)] // Both Linux and macOS
fn posix_test() {
    // ...
}
```

---

## Common Gotchas

### 1. ru_maxrss Units
- **Linux**: kilobytes → multiply by 1024 for bytes
- **macOS**: bytes → use directly
```rust
#[cfg(target_os = "linux")]
let rss_bytes = usage.ru_maxrss as u64 * 1024;

#[cfg(target_os = "macos")]
let rss_bytes = usage.ru_maxrss as u64;
```

### 2. getpriority Return Value
`getpriority` can return -1 as a valid nice value, so always check errno:
```rust
let old_errno = std::io::Error::last_os_error();
let nice = unsafe { getpriority(PRIO_PROCESS, pid) };
let new_errno = std::io::Error::last_os_error();

if nice == -1 && old_errno != new_errno {
    // Real error
    return Err(CapabilityError::Io(new_errno));
}
// nice is valid (could be -1 legitimately)
```

### 3. Process Enumeration Performance
`proc_listallpids` + `proc_pidinfo` for every process can be slow. Consider:
- Caching results
- Filtering early
- Parallel processing for large process lists

### 4. TCC Permissions
Some operations require permissions:
- Full Disk Access for reading other processes' details
- Automation for scripting
Check for permission errors and return `PermissionRequired` with helpful message.

### 5. Page Size
Always get actual page size, don't assume 4096:
```rust
let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
```

---

## Testing Checklist Per Unit

For each unit, verify:

- [ ] Compiles on `aarch64-apple-darwin`
- [ ] Tests pass: `cargo test --target aarch64-apple-darwin`
- [ ] No new clippy warnings
- [ ] Manually verified on real macOS hardware
- [ ] Error cases tested (permission denied, invalid PID, etc.)
- [ ] Compared output to system tools (top, vm_stat, df, netstat, ps)
- [ ] CI check-macos job still passes

---

## Useful System Commands for Verification

```bash
# Get current process stats
ps -o pid,rss,vsz,nice,%cpu,state -p $$

# System-wide CPU
top -l 1 | grep "CPU usage"

# System-wide memory
vm_stat

# All filesystems
df -h

# Network interfaces
netstat -ibn
ifconfig

# All processes with details
ps aux

# Process tree
ps auxww | grep -v grep

# Check permissions
ls -l /var/db/diagnostics  # Example of protected path

# Verify socket
ls -l ~/Library/Application\ Support/ai-ops-coordinator/
```

---

## Emergency Debugging

### Service Won't Start
```bash
# Check if socket already exists
ls -l ~/Library/Application\ Support/ai-ops-coordinator/core.sock

# Remove stale socket
rm ~/Library/Application\ Support/ai-ops-coordinator/core.sock

# Run with RUST_LOG for details
RUST_LOG=debug cargo run -p service
```

### Tests Failing
```bash
# Run single test with output
cargo test --target aarch64-apple-darwin test_name -- --nocapture

# Run with backtraces
RUST_BACKTRACE=1 cargo test --target aarch64-apple-darwin

# Check for leftover test processes
ps aux | grep test
```

### Console Can't Connect
```bash
# Verify service is running
ps aux | grep service

# Check socket exists and has correct permissions
ls -l ~/Library/Application\ Support/ai-ops-coordinator/core.sock

# Test with curl
curl --unix-socket ~/Library/Application\ Support/ai-ops-coordinator/core.sock \
  http://localhost/api/v1/health
```

---

## Reference Documentation Links

### Apple Developer
- [Process Management](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kill.2.html)
- [Virtual Memory](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/ManagingMemory/Articles/AboutMemory.html)
- [IOKit Fundamentals](https://developer.apple.com/library/archive/documentation/DeviceDrivers/Conceptual/IOKitFundamentals/)

### Man Pages
```bash
man 2 getrusage
man 2 getpriority
man 2 statfs
man 3 proc_listallpids
man 3 sysctlbyname
man 1 top
man 1 vm_stat
```

### AsterOpsAI Docs
- Main handoff: `HANDOFF.md`
- macOS units: `docs/macos-development-units.md`
- Mac/Windows transition: `docs/mac-windows-transition-pack.md`
- Linux reference: `docs/linux-build-summary.md`

---

**Keep this file open in a tab while developing — it's your quick reference!**
