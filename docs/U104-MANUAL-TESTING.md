# U104: macOS Console Manual Testing Guide

This document provides a manual testing checklist for verifying the AsterOpsAI Flutter console on macOS desktop.

## Prerequisites

**Service Running**:
```bash
# Verify service is running
lsof /tmp/runtime-501/ai-ops-coordinator/core.sock

# Should show ai-ops-core process listening on the socket
```

**Console Built**:
```bash
# Verify app exists
ls -lh console/build/macos/Build/Products/Release/console.app
```

## Launching the Console

### Option 1: Flutter Run (Development)
```bash
cd console
flutter run -d macos
```

### Option 2: App Bundle (Production)
```bash
open console/build/macos/Build/Products/Release/console.app
```

## Manual Testing Checklist

### ✅ 1. Connection & Startup

- [ ] Console window opens without crashes
- [ ] Connection banner shows "Connected" or connecting state
- [ ] No errors in terminal/console output
- [ ] macOS native window chrome (traffic lights, title bar)

### ✅ 2. Dashboard Screen

- [ ] Overview metrics display
- [ ] No layout issues or overlapping elements
- [ ] Fonts render correctly
- [ ] Navigation drawer accessible

### ✅ 3. CPU Telemetry Screen

- [ ] Aggregate CPU utilization displays
- [ ] Per-core metrics show
- [ ] Load averages (1m, 5m, 15m) visible
- [ ] Pressure indicator (Normal/Elevated/High/Critical)
- [ ] Real-time updates occur

### ✅ 4. Memory Telemetry Screen

- [ ] Total/used/available memory shows
- [ ] Swap usage displays
- [ ] Memory pressure indicator
- [ ] Real-time updates

### ✅ 5. Storage Telemetry Screen

- [ ] Volume list displays
- [ ] Disk usage percentages correct
- [ ] Read/write rates show
- [ ] Multiple volumes (if present) all listed

### ✅ 6. Network Telemetry Screen

- [ ] Network interfaces listed (excluding loopback)
- [ ] RX/TX bytes per second
- [ ] RX/TX packets per second
- [ ] Error rates display

### ✅ 7. Processes Screen

- [ ] Process list displays
- [ ] CPU % per process
- [ ] Memory usage per process
- [ ] Sorting works
- [ ] Search/filter functionality

### ✅ 8. Database Monitoring Screen

- [ ] Database connection status
- [ ] Query performance metrics (if PostgreSQL configured)
- [ ] Graceful handling if no database configured

### ✅ 9. Policy Management Screen

- [ ] Policy inbox displays
- [ ] Pending approvals (if any)
- [ ] Action history
- [ ] Approval workflow functions

### ✅ 10. Security Events Screen

- [ ] Security incidents list
- [ ] Event details
- [ ] Filtering/sorting

### ✅ 11. Settings Screen

**Refresh Interval**:
- [ ] Radio buttons for intervals work
- [ ] Selection persists
- [ ] Data refresh rate changes

**Language Selection**:
- [ ] System default option
- [ ] English option
- [ ] Chinese (Traditional) option
- [ ] UI language changes immediately

**About Section**:
- [ ] Core version displays (if connected)

### ✅ 12. Internationalization (l10n)

- [ ] Switch to Chinese (Traditional)
- [ ] All UI text translates
- [ ] No missing translations
- [ ] Switch back to English
- [ ] Layout adapts to text length

### ✅ 13. Real-Time Updates

- [ ] Telemetry data updates automatically
- [ ] Update interval matches settings
- [ ] No data freezing
- [ ] Connection banner updates on disconnect/reconnect

### ✅ 14. Error Handling

**Test with service stopped**:
```bash
# Kill service
pkill ai-ops-core

# Console should show:
```
- [ ] Connection banner shows "Unavailable" or error
- [ ] Graceful degradation (no crashes)
- [ ] Retry functionality works

**Restart service and verify**:
```bash
target/release/ai-ops-core serve
```
- [ ] Console reconnects automatically
- [ ] Data resumes updating

### ✅ 15. macOS-Specific Features

- [ ] Window resizing works smoothly
- [ ] Maximize/minimize buttons work
- [ ] Full screen mode (green button)
- [ ] Window remembers size/position
- [ ] Cmd+Q quits application
- [ ] Cmd+W closes window
- [ ] Native macOS scrollbars
- [ ] Dark mode support (if system in dark mode)

### ✅ 16. Performance

- [ ] No excessive CPU usage while idle
- [ ] Smooth scrolling in lists
- [ ] No memory leaks (check Activity Monitor)
- [ ] Responsive UI (no lag)

## Known Limitations

1. **Drop Counters**: macOS netstat doesn't expose network drop counters (expected "unavailable")
2. **CPU Frequency**: macOS doesn't expose per-core frequency via Mach APIs (expected "unavailable")
3. **Context Switches/Interrupts**: Not exposed on macOS (expected "unavailable")

## Reporting Issues

If any test fails, document:
1. **Screen/feature** affected
2. **Expected behavior**
3. **Actual behavior**
4. **Console output** (if running via `flutter run`)
5. **System info**: macOS version, architecture (arm64/x86_64)

## Success Criteria

- All 16 sections above pass
- No crashes or errors
- Data displays correctly and updates in real-time
- UI is responsive and follows macOS design patterns
- Internationalization works for all supported languages

---

**Status**: Infrastructure verified ✅ | Manual GUI testing pending ⏸️
