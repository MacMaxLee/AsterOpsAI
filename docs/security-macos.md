# macOS Security & Permissions

This document explains macOS-specific security considerations, required permissions, and privacy implications for AsterOpsAI.

## Executive Summary

**TL;DR**: AsterOpsAI works out-of-the-box on macOS **without requiring any special permissions** for basic telemetry. Full Disk Access may improve process information in some cases, but is not required for normal operation.

✅ **Works without any permissions**:
- CPU telemetry (usage, load averages, pressure)
- Memory telemetry (RAM, swap, pressure)
- Storage telemetry (disk capacity, free space)
- Network telemetry (interface statistics, rates)
- Process enumeration (PIDs, names, CPU%, memory)

⚠️ **May need Full Disk Access** (optional):
- Executable paths for processes owned by other users (rare)
- Detailed process information for system-protected processes

## macOS Security Model

### Transparency, Consent, and Control (TCC)

macOS uses a privacy framework called TCC (Transparency, Consent, and Control) to protect sensitive user data and system resources. Starting with macOS 10.14 Mojave, certain operations trigger permission prompts or require explicit grants in System Settings.

**AsterOpsAI's approach**: Use **publicly available system APIs** that don't trigger TCC prompts for normal operation.

### System Integrity Protection (SIP)

macOS System Integrity Protection (SIP) prevents modification of protected system files and processes, even with root privileges. AsterOpsAI:
- ✅ **Does not** attempt to modify system files
- ✅ **Does not** require SIP to be disabled
- ✅ **Does not** need elevated (root) privileges
- ✅ Operates entirely in user space with standard APIs

## Telemetry Permissions Breakdown

### CPU Telemetry

**APIs Used**:
- `host_statistics64(HOST_CPU_LOAD_INFO)` — Mach kernel API
- `host_processor_info()` — Per-core statistics
- `getloadavg(3)` — System load averages

**Required Permissions**: **None**

**TCC Prompt**: **Never**

**How It Works**: These are public kernel APIs available to all user-space processes. No special entitlements or permissions required.

**Degradation**: None. All CPU telemetry works without any permissions.

### Memory Telemetry

**APIs Used**:
- `sysctl hw.memsize` — Total RAM
- `sysctl hw.pagesize` — Memory page size
- `sysctl vm.swapusage` — Swap usage statistics
- `host_statistics64(HOST_VM_INFO64)` — VM statistics

**Required Permissions**: **None**

**TCC Prompt**: **Never**

**How It Works**: sysctl and Mach APIs are public interfaces. All memory information is available to any process.

**Degradation**: None. All memory telemetry works without any permissions.

### Storage Telemetry

**APIs Used**:
- `getfsstat(2)` — List all mounted filesystems
- `statfs(2)` — Per-filesystem capacity and free space

**Required Permissions**: **None**

**TCC Prompt**: **Never**

**How It Works**: Filesystem statistics are public information on macOS. External drives, network volumes, and APFS volumes are all visible.

**Limitations** (API-level, not permissions):
- Disk I/O metrics (reads/writes per second) are **not implemented** (deferred in Milestone 3)
- Would require IOKit APIs if implemented in the future
- See `docs/performance-macos.md` for rationale

**Degradation**: None for implemented features.

### Network Telemetry

**APIs Used**:
- `netstat -ibn` command execution via `std::process::Command`

**Required Permissions**: **None**

**TCC Prompt**: **Never**

**How It Works**: The `netstat` command is a standard macOS utility available to all users. Interface statistics (bytes/packets received/transmitted) are public information.

**Limitations** (API-level, not permissions):
- Drop counters not exposed by macOS `netstat` (see `telemetry_macos/network.rs` docs)

**Degradation**: None for implemented features.

### Process Telemetry

**APIs Used**:
- `proc_listpids(PROC_ALL_PIDS)` — List all process IDs
- `proc_pidinfo(PROC_PIDTASKINFO)` — Per-process CPU time, memory
- `proc_pidinfo(PROC_PIDTBSDINFO)` — Owner UID, process name
- `proc_pidpath()` — Executable path

**Required Permissions**: **Usually none**, **Full Disk Access** in rare cases

**TCC Prompt**: **Usually never**, **may appear** for certain processes

**How It Works**:
- Process enumeration (`proc_listpids`) **always works** — macOS allows any process to see all PIDs
- Basic info (`proc_pidinfo`) **usually works** — CPU%, memory, owner UID, process name accessible for most processes
- Executable paths (`proc_pidpath`) **may fail** for processes owned by other users or system-protected processes

**When Full Disk Access Helps**:
1. **Executable paths for other users' processes**: Without Full Disk Access, `proc_pidpath()` may return empty for processes not owned by you
2. **System-protected processes**: Some macOS system processes are protected and require Full Disk Access to read detailed info

**What Still Works Without Full Disk Access**:
- ✅ Process enumeration (all PIDs visible)
- ✅ Process names (comm field from PROC_PIDTBSDINFO)
- ✅ CPU percentage (calculated from nanosecond time deltas)
- ✅ Memory usage (RSS bytes)
- ✅ Owner UID (which user owns the process)
- ✅ Process classification (System/UserApplication/BackgroundService)

**What May Be Missing Without Full Disk Access**:
- ⚠️ Executable paths (`/Applications/Safari.app/Contents/MacOS/Safari`) for processes owned by other users
- ⚠️ Command-line arguments (not implemented yet, would require `sysctl KERN_PROCARGS2`)

**Graceful Degradation**: AsterOpsAI uses `MetricValue::Unavailable { reason }` for missing data rather than failing. The API will return:
```json
{
  "exe_path": {
    "type": "unavailable",
    "reason": "permission denied or process protected"
  }
}
```

## Granting Permissions (If Needed)

### Full Disk Access (Optional)

If you want complete executable path information for all processes, grant Full Disk Access:

#### Via System Settings

1. Open **System Settings** (macOS 13+) or **System Preferences** (macOS 12 and earlier)
2. Navigate to **Privacy & Security** → **Full Disk Access**
3. Click the **lock icon** to unlock (requires admin password)
4. Click the **+** button
5. Navigate to the AsterOpsAI service binary:
   - If running from source: `~/path/to/AsterOpsAI/target/release/ai-ops-core`
   - If installed: `/usr/local/bin/ai-ops-core` (future)
6. Check the box next to the added binary
7. Restart the AsterOpsAI service for changes to take effect

#### Via Terminal (GUI prompt will appear)

macOS may automatically prompt for Full Disk Access the first time AsterOpsAI tries to read protected process information. If prompted:

1. Click **Open System Settings** in the prompt
2. Follow the steps above
3. Restart the service

### Verifying Permissions

To check if AsterOpsAI has Full Disk Access:

```bash
# Check if Terminal has Full Disk Access (if running from terminal)
sqlite3 "file://$(getconf DARWIN_USER_DIR)TCC.db" "SELECT service, client FROM access WHERE service='kTCCServiceSystemPolicyAllFiles';" 2>/dev/null
```

**Note**: This is a diagnostic command. If it returns results, Full Disk Access may be granted to your terminal emulator.

## Unix Socket Security

### Permission Model

AsterOpsAI creates a Unix domain socket with strict permissions:

- **Socket file**: `mode 0600` (owner read/write only)
- **Parent directory**: `mode 0700` (owner access only)
- **Location**: `$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock`
  - Typically: `/tmp/runtime-$(id -u)/ai-ops-coordinator/core.sock`

**Security Properties**:
- ✅ **Only your user account** can connect to the service
- ✅ **Other users** on the same machine **cannot** access your telemetry data
- ✅ **No network exposure** — Unix sockets are local-only (cannot be accessed remotely)
- ✅ **Automatic cleanup** — `/tmp` is cleared on reboot, removing stale sockets

### Verifying Socket Permissions

```bash
ls -lh "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock"
# Expected output: srw------- (socket, owner-only)

ls -lhd "$XDG_RUNTIME_DIR"
# Expected output: drwx------ (directory, owner-only)
```

If permissions are incorrect:
```bash
chmod 700 "$XDG_RUNTIME_DIR"
chmod 600 "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock"
```

### Multi-User Considerations

**Current Behavior** (Developer/Single-User Deployment):
- Socket is user-specific (`/tmp/runtime-$(id -u)`)
- Each user running AsterOpsAI gets their own isolated socket
- No cross-user access possible

**Future** (System-Wide Deployment):
- Would use a shared socket location (e.g., `/var/run/ai-ops-coordinator/core.sock`)
- Would require `SMAppService` privileged helper (macOS 13+)
- Would implement access control via Unix groups or TCC entitlements
- **Not implemented in v1** (deferred to production deployment guide)

## Process Control Permissions

### Changing Process Priority (`nice`, `renice`)

**API Used**: `setpriority(PRIO_PROCESS, pid, nice_value)`

**Required Permissions**:
- ✅ **Own processes**: No permissions required
- ❌ **Other users' processes**: Requires root (not supported)

**How It Works**: macOS allows any process to adjust the priority of processes it owns (same effective UID). Attempting to change another user's process priority returns `EPERM` (Operation not permitted) or `EACCES` (Permission denied).

**AsterOpsAI Behavior**:
- Attempts `setpriority()` for the target process
- If `EPERM` or `EACCES`: Returns `CapabilityError::PermissionRequired`
- Graceful degradation: API returns error, doesn't crash service

**Frontend Handling**: The console should display:
```
Failed to change process priority: Permission required (process owned by another user)
```

### Terminating Processes (`kill`)

**API Used**: `kill(pid, SIGTERM)` or `kill(pid, SIGKILL)`

**Required Permissions**:
- ✅ **Own processes**: No permissions required
- ❌ **Other users' processes**: Requires root (not supported)

**AsterOpsAI Behavior**:
- Sends signal to target process via `kill(2)`
- If `EPERM`: Returns `CapabilityError::PermissionRequired`
- Graceful degradation: API returns error, doesn't crash service

**Security Note**: AsterOpsAI **never runs as root** and **never requests root privileges**. Process control is limited to processes owned by the same user running the service.

## Permissions AsterOpsAI Does NOT Need

macOS has many TCC permission categories. AsterOpsAI explicitly **does not need** and **does not request**:

- ❌ **Accessibility** — No GUI scripting or accessibility API usage
- ❌ **Input Monitoring** — No keyboard/mouse capture
- ❌ **Screen Recording** — No screen capture
- ❌ **Camera** — No camera access
- ❌ **Microphone** — No audio recording
- ❌ **Location Services** — No geolocation
- ❌ **Contacts, Calendar, Reminders** — No PIM data access
- ❌ **Photos** — No photo library access
- ❌ **Automation** — No AppleScript control of other apps (except `netstat` command execution)

**Design Principle**: AsterOpsAI is a **non-invasive monitoring tool**. It observes public system metrics without requesting sensitive user data.

## Privacy Considerations

### What Data Does AsterOpsAI Collect?

**System Telemetry** (no personally identifiable information):
- CPU usage percentages, load averages
- Memory usage, swap usage
- Disk capacity, free space
- Network interface statistics (bytes/packets, no packet content)

**Process Information** (may include process names):
- Process IDs (PIDs)
- Process names (e.g., "Safari", "Mail", "Xcode")
- Executable paths (e.g., `/Applications/Safari.app/Contents/MacOS/Safari`)
- CPU and memory usage per process
- Owner UID (numeric user ID, not username)

**What AsterOpsAI Does NOT Collect**:
- ❌ Process arguments (command-line flags, file paths in arguments)
- ❌ Network packet content or payloads
- ❌ File contents or file listings
- ❌ User input (keyboard, mouse)
- ❌ Screen contents or screenshots
- ❌ Personally identifiable information (PII)

### Data Storage

**Local SQLite Database** (if `--db-path` specified):
- Telemetry snapshots stored locally on disk
- Database file location: user-controlled
- Encrypted: No (future enhancement)
- Network transmission: No (service is local-only via Unix socket)

**Network Exposure**: **None**
- Service binds to Unix domain socket only (not TCP)
- No cloud services, no external API calls
- All data stays on your machine

### Compliance

**GDPR/Privacy Laws**: AsterOpsAI:
- Operates entirely locally (no data transmission to external servers)
- Does not collect PII by design
- User controls all data (can delete SQLite database at any time)
- Suitable for use in privacy-sensitive environments

**Corporate/Enterprise Use**: System administrators should:
- Review process information exposure (process names may reveal user activity)
- Consider access controls for the Unix socket in multi-user environments
- Evaluate data retention policies for SQLite database

## Future: System-Wide Deployment (Not Implemented)

### SMAppService Helper (macOS 13+)

For system-wide deployment (monitoring all users, running at boot), AsterOpsAI would require:

1. **SMAppService Privileged Helper**:
   - Apple-signed helper tool running as root or `_windowserver` user
   - Installed via `SMAppService` API (replacement for deprecated launchd helpers)
   - Requires code signing with valid Apple Developer ID

2. **TCC Entitlements**:
   - `com.apple.private.tcc.allow` entitlement for Full Disk Access
   - Notarization required for Gatekeeper approval

3. **Launch Daemon**:
   - Plist at `/Library/LaunchDaemons/com.asterops.service.plist`
   - Socket at `/var/run/ai-ops-coordinator/core.sock` (shared, system-wide)

**Status**: **Not implemented** in v1. Deferred to production deployment guide (post-v1).

**Reference**: TRS §39 identifies TCC requirements for privileged operations.

## Troubleshooting

### "Permission denied" When Reading Process Info

**Symptom**: Some processes show `exe_path: unavailable` or missing details.

**Cause**: macOS protects certain system processes from inspection without Full Disk Access.

**Solutions**:
1. **Accept graceful degradation** — Basic process info (name, PID, CPU%, memory) still works
2. **Grant Full Disk Access** — Follow steps above to grant permission

### "Operation not permitted" When Changing Process Priority

**Symptom**: `setpriority()` fails with `EPERM` or `EACCES`.

**Cause**: Attempting to change priority of a process owned by another user.

**Solution**: This is expected behavior. AsterOpsAI can only adjust priority for processes owned by the same user running the service. No fix needed — this is a security feature.

### No Telemetry Data Returned

**Symptom**: API endpoints return empty or minimal data.

**Possible Causes**:
1. **Service not running**: Check `ps aux | grep ai-ops-core`
2. **Socket permissions incorrect**: Check `ls -lh $XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock`
3. **XDG_RUNTIME_DIR not set**: Ensure environment variable is configured (see `docs/INSTALL-MACOS.md`)

**Debugging**:
```bash
# Check service logs
tail -f /tmp/asterops.log  # If running in background

# Test socket connectivity
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" http://localhost/api/v1/health

# Verify telemetry
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" http://localhost/api/v1/cpu | jq
```

## Conclusion

AsterOpsAI is designed for **zero-friction macOS deployment**:
- ✅ No special permissions required for normal operation
- ✅ No TCC prompts for standard telemetry
- ✅ Graceful degradation when permissions are missing
- ✅ Private by design (local-only, no network exposure)

For most users, **no configuration is needed** — just build and run. Full Disk Access is optional and only improves edge-case process information.

For enterprise/production deployment with system-wide monitoring, see future production deployment guide (post-v1).

## Related Documentation

- **Installation**: `docs/INSTALL-MACOS.md`
- **Architecture**: `docs/ARCHITECTURE.md`
- **Performance**: `docs/performance-macos.md`
- **Technical Requirements**: `docs/TRS.md` (§39 for TCC requirements)
