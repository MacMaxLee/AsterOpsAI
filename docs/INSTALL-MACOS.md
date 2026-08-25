# Installing AsterOpsAI on macOS

This guide walks through building and running AsterOpsAI on macOS. Designed for developers and administrators setting up a local monitoring environment.

## Prerequisites

### 1. Xcode Command Line Tools

Required for Rust compilation and system libraries.

```bash
xcode-select --install
```

If already installed, verify with:
```bash
xcode-select -p
# Should output: /Library/Developer/CommandLineTools or /Applications/Xcode.app/Contents/Developer
```

### 2. Rust Toolchain

Install via rustup (recommended):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts, then restart your terminal or run:
```bash
source $HOME/.cargo/env
```

Verify installation:
```bash
rustc --version
cargo --version
```

Expected: Rust 1.70+ (workspace requires stable toolchain).

### 3. Flutter (Optional - for GUI Console)

Required only if you want to run the graphical console application.

```bash
# Install via Homebrew
brew install flutter

# Or download from flutter.dev
```

Enable macOS desktop support:
```bash
flutter config --enable-macos-desktop
flutter doctor
```

### 4. PostgreSQL (Optional - for Database Monitoring)

Required only if you want to monitor PostgreSQL instances.

```bash
# Install via Homebrew
brew install postgresql@15
brew services start postgresql@15

# Or use Postgres.app from https://postgresapp.com
```

## Building from Source

### 1. Clone the Repository

```bash
git clone https://github.com/your-org/AsterOpsAI.git
cd AsterOpsAI
```

### 2. Build the Workspace

Build all Rust crates:

```bash
cargo build --workspace --release
```

This compiles:
- `contracts` - Wire types and schemas
- `platform` - macOS platform adapter
- `core` - Telemetry, persistence, analysis
- `service` - HTTP API server binary

Build artifacts will be in `target/release/`.

**Build time**: ~5-10 minutes on first build (downloads dependencies), ~1-2 minutes on subsequent builds.

### 3. Build the Console (Optional)

If you want the Flutter GUI:

```bash
cd console
flutter pub get
flutter build macos --release
```

The macOS app bundle will be created at:
```
build/macos/Build/Products/Release/console.app
```

## Running the Service

### 1. Set Up Runtime Directory

macOS doesn't have a standard `XDG_RUNTIME_DIR` like Linux. AsterOpsAI uses `/tmp/runtime-$(id -u)` by convention:

```bash
export XDG_RUNTIME_DIR="/tmp/runtime-$(id -u)"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
```

**Recommendation**: Add this to your shell profile (`~/.zshrc` or `~/.bash_profile`) to persist across sessions:

```bash
# Add to ~/.zshrc
export XDG_RUNTIME_DIR="/tmp/runtime-$(id -u)"
[ ! -d "$XDG_RUNTIME_DIR" ] && mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
```

### 2. Run the Service

From the repository root:

```bash
./target/release/ai-ops-core serve
```

You should see:
```
INFO ai_ops_service::transport::unix: listening on unix domain socket path=/tmp/runtime-501/ai-ops-coordinator/core.sock
```

**Note**: The numeric suffix (e.g., `501`) is your user ID (`id -u`).

### 3. Verify It's Running

In another terminal, test the health endpoint:

```bash
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/health
```

Expected response:
```json
{"status":"ok"}
```

Test real telemetry (macOS-specific):

```bash
# CPU telemetry
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/cpu | jq

# Memory telemetry
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/memory | jq

# All processes
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/processes | jq
```

### 4. Run in Background (Optional)

To run as a background service:

```bash
nohup ./target/release/ai-ops-core serve > /tmp/asterops.log 2>&1 &
echo $! > /tmp/asterops.pid
```

To stop:
```bash
kill $(cat /tmp/asterops.pid)
```

## Running the Console (Optional)

If you built the Flutter console:

### Option 1: From Xcode

```bash
open console/build/macos/Build/Products/Release/console.app
```

### Option 2: From Flutter

```bash
cd console
flutter run -d macos --release
```

The console will automatically connect to the service via the Unix socket.

## Unix Socket Security

### Permissions

AsterOpsAI creates the Unix socket with **mode 0600** (owner read/write only). This means:

- ✅ Only your user account can connect to the service
- ✅ Other users on the same machine cannot access your telemetry data
- ✅ No network exposure (Unix sockets are local-only)

Verify socket permissions:
```bash
ls -lh "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock"
# Should show: srw-------  (socket, owner-only)
```

### Runtime Directory

The `$XDG_RUNTIME_DIR` directory is created with **mode 0700** (owner-only access). This provides:

- Directory-level isolation
- Protection against unauthorized socket creation
- Standard Unix security model

## Connecting to PostgreSQL (Optional)

If you want to monitor PostgreSQL databases:

### 1. Configure PostgreSQL Connection

The service will look for PostgreSQL connection details via environment variables or CLI flags:

```bash
# Example: Monitor local PostgreSQL
./target/release/ai-ops-core serve \
  --pg-host localhost \
  --pg-port 5432 \
  --pg-user asterops \
  --pg-dbname postgres
```

**Note**: Database monitoring features are planned for Milestone 6 (not yet implemented).

### 2. Test PostgreSQL Connection

Once database monitoring is implemented, you'll be able to query:

```bash
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/database/status
```

## Troubleshooting

### Service Won't Start

**Problem**: `XDG_RUNTIME_DIR is not set; refusing to guess a socket location`

**Solution**: Set the environment variable:
```bash
export XDG_RUNTIME_DIR="/tmp/runtime-$(id -u)"
mkdir -p "$XDG_RUNTIME_DIR"
```

---

**Problem**: `Address already in use` when starting service

**Solution**: Another instance is running, or socket file is stale:
```bash
# Check for running process
ps aux | grep ai-ops-core

# Remove stale socket
rm "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock"
```

---

**Problem**: `Permission denied` when accessing socket

**Solution**: Check socket permissions and ownership:
```bash
ls -lh "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock"
# Should be owned by your user, mode 0600
```

### Build Failures

**Problem**: `error: linker 'cc' not found`

**Solution**: Install Xcode Command Line Tools:
```bash
xcode-select --install
```

---

**Problem**: `error: could not find native library 'sqlite3'`

**Solution**: Install SQLite via Homebrew:
```bash
brew install sqlite
```

---

**Problem**: Compilation errors in `telemetry_macos` modules

**Solution**: Verify you're on a supported macOS version (10.15+):
```bash
sw_vers
```

### Console Won't Connect

**Problem**: Console shows "Disconnected" or "Unable to connect"

**Solution**: Verify the service is running and socket exists:
```bash
# Check service is running
ps aux | grep ai-ops-core

# Check socket exists
ls -lh "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock"

# Test with curl
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/health
```

---

**Problem**: Flutter build fails with dependency errors

**Solution**: Update Flutter and clean build cache:
```bash
flutter upgrade
flutter clean
cd console
flutter pub get
flutter build macos
```

### Telemetry Issues

**Problem**: Empty process list or missing data

**Solution**: macOS may require Full Disk Access for process information:

1. Open **System Settings** → **Privacy & Security** → **Full Disk Access**
2. Add `Terminal.app` or your terminal emulator
3. Restart the service

**Note**: For most telemetry, Full Disk Access is NOT required. Process names and CPU usage work without elevated permissions.

---

**Problem**: Network telemetry shows zeros

**Solution**: Verify `netstat` is available:
```bash
which netstat
netstat -ibn
```

If missing, install via Xcode Command Line Tools.

---

**Problem**: Storage telemetry doesn't show external drives

**Solution**: External drives are supported. Verify mount points:
```bash
df -h
# Should show all mounted filesystems
```

AsterOpsAI filters pseudo-filesystems (`devfs`, `autofs`) but includes real volumes.

## Performance

Based on benchmarks (see `docs/performance-macos.md`):

- **Full telemetry cycle**: ~3ms on Apple M4 Pro
- **CPU overhead**: <0.3% with 1-second sampling
- **Memory footprint**: ~10-20 MB (service only)

The service is designed for continuous operation with minimal resource usage.

## Next Steps

- **Read the SRS**: See `docs/SRS.md` for feature overview
- **Explore the API**: All endpoints documented in `schemas/*.schema.json`
- **Configure sampling**: See `docs/ARCHITECTURE.md` for adaptive sampling behavior
- **Set up database monitoring**: (Coming in Milestone 6)

## Getting Help

- **Issue Tracker**: https://github.com/your-org/AsterOpsAI/issues
- **Architecture Docs**: `docs/ARCHITECTURE.md`
- **Technical Requirements**: `docs/TRS.md`
- **Development Guide**: `CLAUDE.md`

## Security Considerations

See `docs/security-macos.md` (coming in U109) for:
- macOS Transparency, Consent, and Control (TCC) permissions
- Full Disk Access requirements
- Unix socket security model
- Future: SMAppService helper for system-wide deployment
