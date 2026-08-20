#!/usr/bin/env bash
# One-time root setup for the storage-latency demo scenario (unit U21):
# delegates the cgroup v2 `io` controller into a throttled child cgroup
# this user can manage without further root access, so
# storage-latency.sh can apply a *genuine* kernel-level I/O bandwidth cap
# (cgroup v2 io.max) to its own stress workload. Real dd-based
# contention alone was tested and confirmed insufficient on fast
# virtualized storage (see docs/adr/0026) — io.max makes the induced
# slowdown real (the kernel actually throttles it), not simulated.
#
# Must be run with sudo. Idempotent: safe to re-run.
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  echo "error: run this with sudo" >&2
  exit 1
fi

TARGET_USER="${SUDO_USER:-${1:?usage: sudo $0 <username> (or run via sudo so SUDO_USER is set)}}"
CGROUP_ROOT="/sys/fs/cgroup"
DEMO_CGROUP="$CGROUP_ROOT/asterops-demo-io"

if ! grep -qw io "$CGROUP_ROOT/cgroup.controllers"; then
  echo "error: the io controller isn't available on this system's cgroup v2 hierarchy" >&2
  exit 1
fi

# Enable the io controller for children of the root cgroup, if not
# already enabled.
if ! grep -qw io "$CGROUP_ROOT/cgroup.subtree_control"; then
  echo "io" >"$CGROUP_ROOT/cgroup.subtree_control"
fi

mkdir -p "$DEMO_CGROUP"
chown -R "$TARGET_USER" "$DEMO_CGROUP"

echo "ok: $DEMO_CGROUP is delegated to $TARGET_USER"
echo "    (scripts/demo/storage-latency.sh writes its own io.max limit and"
echo "     moves its stress processes' PIDs into this cgroup at run time —"
echo "     no further root access needed after this)."
