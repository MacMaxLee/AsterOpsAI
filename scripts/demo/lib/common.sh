#!/usr/bin/env bash
# Shared shell library for the U21 demo scenarios (lock-storm.sh,
# pool-exhaustion.sh, storage-latency.sh). Not executable directly —
# sourced by each scenario script. Reuses the same extracted-binary,
# no-Docker PostgreSQL technique already proven three times in this
# project (scripts/setup-test-postgres.sh -> core/tests/dbms/common/
# mod.rs -> service/tests/support/mod.rs); reimplemented once more in
# bash here since a shell script can't import a Rust test harness (same
# accepted duplication ADR 0025 already documented).
set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
# Deliberately NOT under $REPO_ROOT: a Unix domain socket path is capped
# at ~108 bytes by the kernel (`sun_path`/SUN_LEN) — nesting this under
# the repo's own (long) absolute path blew past that limit the first
# time this ran for real. Real systems keep XDG_RUNTIME_DIR short for
# exactly this reason; this does the same.
STATE_ROOT="${ASTEROPS_DEMO_STATE_ROOT:-${TMPDIR:-/tmp}/asterops-demo}"
PG_INSTALL_ROOT="${PG_TEST_INSTALL_ROOT:-$HOME/.local-tools}"
PG_MAJOR_VERSION="${PG_MAJOR_VERSION:-17}"

pg_bin_dir() {
  echo "$PG_INSTALL_ROOT/pg${PG_MAJOR_VERSION}/usr/lib/postgresql/${PG_MAJOR_VERSION}/bin"
}

pg_lib_dir() {
  find "$PG_INSTALL_ROOT/pg-shared" -iname 'libpq.so.5' 2>/dev/null | head -1 | xargs -r dirname
}

require_pg_binaries() {
  local bin_dir
  bin_dir="$(pg_bin_dir)"
  if [ ! -x "$bin_dir/initdb" ]; then
    echo "error: PostgreSQL ${PG_MAJOR_VERSION} binaries not found at $bin_dir" >&2
    echo "run scripts/setup-test-postgres.sh first" >&2
    exit 1
  fi
}

scenario_dir() {
  echo "$STATE_ROOT/$1"
}

track_pid() {
  local scenario="$1" label="$2" pid="$3"
  mkdir -p "$(scenario_dir "$scenario")/pids"
  echo "$pid" >"$(scenario_dir "$scenario")/pids/$label.pid"
}

# Kills exactly the PIDs this demo tracked for one scenario (if still
# alive) and removes that scenario's scratch directory. Safe to call on a
# scenario that was never started, or already torn down. Never touches
# anything not tracked here — no broad pkill of postgres/service/console
# processes system-wide.
cleanup_scenario() {
  local scenario="$1"
  local dir
  dir="$(scenario_dir "$scenario")"
  if [ -d "$dir/pids" ]; then
    local pid_file pid waited
    for pid_file in "$dir"/pids/*.pid; do
      [ -e "$pid_file" ] || continue
      pid="$(cat "$pid_file")"
      if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        waited=0
        while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt 20 ]; do
          sleep 0.2
          waited=$((waited + 1))
        done
        kill -9 "$pid" 2>/dev/null || true
      fi
    done
  fi
  rm -rf "$dir"
}

# $1 scenario name; remaining args are extra `-c key=value` postmaster
# options appended verbatim (e.g. to lower max_connections for the
# pool-exhaustion scenario). Sets DEMO_PG_SOCKET_DIR / DEMO_PG_PORT /
# DEMO_PG_BIN_DIR / DEMO_PG_LIB_DIR on success. Must be called via
# `source`/in the current shell, not a subshell, so those exports stick.
start_postgres() {
  local scenario="$1"
  shift
  require_pg_binaries
  local dir bin_dir lib_dir port
  dir="$(scenario_dir "$scenario")"
  bin_dir="$(pg_bin_dir)"
  lib_dir="$(pg_lib_dir)"
  mkdir -p "$dir"
  port="$(python3 -c 'import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()')"

  echo "  starting throwaway PostgreSQL ${PG_MAJOR_VERSION} for '$scenario' on port $port..."
  LD_LIBRARY_PATH="$lib_dir" "$bin_dir/initdb" -D "$dir/pgdata" -U postgres --auth=trust \
    >"$dir/initdb.log" 2>&1

  local extra_opts="$*"
  LD_LIBRARY_PATH="$lib_dir" "$bin_dir/pg_ctl" -D "$dir/pgdata" -l "$dir/postgres.log" \
    -o "-p $port -k $dir -c listen_addresses='' $extra_opts" start >"$dir/pg_ctl.log" 2>&1

  local pg_pid
  pg_pid="$(head -1 "$dir/pgdata/postmaster.pid")"
  track_pid "$scenario" postgres "$pg_pid"

  local deadline=$((SECONDS + 15))
  while ! LD_LIBRARY_PATH="$lib_dir" "$bin_dir/pg_isready" -h "$dir" -p "$port" >/dev/null 2>&1; do
    if [ "$SECONDS" -ge "$deadline" ]; then
      echo "error: postgres did not become ready in time; see $dir/postgres.log" >&2
      exit 1
    fi
    sleep 0.1
  done

  export DEMO_PG_SOCKET_DIR="$dir"
  export DEMO_PG_PORT="$port"
  export DEMO_PG_BIN_DIR="$bin_dir"
  export DEMO_PG_LIB_DIR="$lib_dir"
  export LD_LIBRARY_PATH="$lib_dir"
}

# A real, synchronous (foreground) psql invocation against the scenario's
# throwaway instance — for anything backgrounded/tracked, use
# demo_psql_argv + `( exec ... ) &` instead (see its own comment for why).
demo_psql() {
  local scenario="$1"
  shift
  local dir
  dir="$(scenario_dir "$scenario")"
  "$DEMO_PG_BIN_DIR/psql" -h "$dir" -p "$DEMO_PG_PORT" -U postgres -d postgres \
    -v ON_ERROR_STOP=1 "$@"
}

# Populates the global array DEMO_PSQL_ARGV with the psql invocation for
# `scenario`, for a caller that needs to background *and reliably track
# the real PID of* a long-lived psql session (a lock holder, an idle
# connection). Backgrounding a bash *function* call (`demo_psql ... &`)
# captures the wrapping subshell's PID via `$!`, not the actual `psql`
# leaf process underneath it — killing that PID later leaves psql itself
# orphaned (a real bug found running this scenario for real: `reset.sh`
# left a holder session alive). The fix is the standard bash idiom:
# `( exec "${DEMO_PSQL_ARGV[@]}" ... ) &` — `exec` replaces the subshell
# process image with psql in place, so `$!` becomes psql's own real PID.
demo_psql_argv() {
  local scenario="$1"
  local dir
  dir="$(scenario_dir "$scenario")"
  DEMO_PSQL_ARGV=(
    "$DEMO_PG_BIN_DIR/psql" -h "$dir" -p "$DEMO_PG_PORT" -U postgres -d postgres
    -v ON_ERROR_STOP=1
  )
}

build_service_release() {
  if [ ! -x "$REPO_ROOT/target/release/ai-ops-core" ]; then
    echo "  building service (release, one-time — this may take a minute)..."
    (cd "$REPO_ROOT/rust_core" && cargo build --release -p service)
  fi
}

console_bundle_path() {
  find "$REPO_ROOT/console/build/linux" -maxdepth 4 -type f -name console \
    -path '*/release/bundle/*' 2>/dev/null | head -1
}

require_console_bundle() {
  local bundle
  bundle="$(console_bundle_path)"
  if [ -z "$bundle" ]; then
    echo "error: console isn't built yet — run scripts/demo/build-console.sh first" >&2
    exit 1
  fi
}

# Starts `service` pointed at the scenario's throwaway Postgres
# (DEMO_PG_SOCKET_DIR/DEMO_PG_PORT, set by start_postgres). Sets
# DEMO_RUNTIME_DIR / DEMO_SOCKET_PATH on success.
start_service() {
  local scenario="$1"
  build_service_release
  local dir
  dir="$(scenario_dir "$scenario")"
  mkdir -p "$dir/runtime/ai-ops-coordinator" "$dir/data"

  echo "  starting service for '$scenario'..."
  (
    export XDG_RUNTIME_DIR="$dir/runtime"
    export XDG_DATA_HOME="$dir/data"
    export ASTEROPS_DB_HOST="$DEMO_PG_SOCKET_DIR"
    export ASTEROPS_DB_PORT="$DEMO_PG_PORT"
    export ASTEROPS_DB_DATABASE="postgres"
    export ASTEROPS_DB_USER="postgres"
    export ASTEROPS_DB_PASSWORD="trust-auth-ignores-this"
    export ASTEROPS_DB_SSLMODE="disable"
    exec "$REPO_ROOT/target/release/ai-ops-core"
  ) >"$dir/service.log" 2>&1 &
  local svc_pid=$!
  track_pid "$scenario" service "$svc_pid"

  local sock="$dir/runtime/ai-ops-coordinator/core.sock"
  local deadline=$((SECONDS + 15))
  while [ ! -S "$sock" ]; do
    if [ "$SECONDS" -ge "$deadline" ]; then
      echo "error: service did not create its socket in time; see $dir/service.log" >&2
      exit 1
    fi
    sleep 0.1
  done

  export DEMO_RUNTIME_DIR="$dir/runtime"
  export DEMO_SOCKET_PATH="$sock"
}

launch_console() {
  local scenario="$1"
  require_console_bundle
  local dir bundle
  dir="$(scenario_dir "$scenario")"
  bundle="$(console_bundle_path)"

  echo "  launching console for '$scenario'..."
  (
    export XDG_RUNTIME_DIR="$dir/runtime"
    exec "$bundle"
  ) >"$dir/console.log" 2>&1 &
  local console_pid=$!
  track_pid "$scenario" console "$console_pid"
}

# Polls the real correlation endpoint until it matches
# demo/scenarios/<scenario>/expected-verdict.json or times out.
check_verdict() {
  local scenario="$1" timeout="${2:-90}"
  python3 "$REPO_ROOT/scripts/demo/lib/check_verdict.py" \
    --socket "$DEMO_SOCKET_PATH" \
    --expected "$REPO_ROOT/demo/scenarios/$scenario/expected-verdict.json" \
    --timeout "$timeout"
}
