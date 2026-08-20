#!/usr/bin/env bash
# Storage latency — slow disk, healthy database (unit U21, REQUIREMENTS
# #1c). Single command: provisions a real, idle (healthy) throwaway
# PostgreSQL, generates real concurrent O_DIRECT disk contention against
# a small bounded scratch file, starts service + the console, and checks
# the verdict against demo/scenarios/storage-latency/expected-
# verdict.json.
#
# Honesty note (see docs/adr/0026 and docs/demo/RUNBOOK.md): unprivileged
# dd-based contention was confirmed, empirically, NOT to reliably cross
# the real STORAGE_IO_LATENCY_MS_HIGH/CRITICAL thresholds (30ms/80ms) on
# fast virtualized/SSD-backed storage — this scenario may FAIL its
# expected-verdict check on such a machine through no fault of the
# correlation engine. It's shipped anyway because the mechanism is real
# (genuine /proc/diskstats-measured contention, not simulated) and will
# behave correctly on storage that's actually slow enough to queue.
set -euo pipefail

# shellcheck source=lib/common.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

SCENARIO="storage-latency"
START=$SECONDS
SCRATCH_MB=512
# Capped at nproc, and each reader does a bounded number of *large*
# sequential passes (not a tight per-4KB-block bash loop) — a real
# discovery while building this scenario: a `while true; do dd ...;
# done` loop respawning dd thousands of times a second is CPU-bound from
# fork/exec overhead alone, not disk-bound. On this sandbox's 2 CPUs
# that alone was enough to make the correlation engine correctly report
# HOST_CPU instead — an accurate diagnosis of what was actually
# happening, but not what this scenario means to demonstrate, and it
# contaminates the "healthy host" half of the scenario's own premise.
# Fewer, larger dd invocations keep the load-generation technique itself
# genuinely I/O-bound (confirmed via `top` showing real iowait, ~0%
# user CPU) rather than becoming the bottleneck it reports.
READERS=$(nproc)
PASSES_PER_READER=8
CONTENTION_SECONDS=28

echo "== storage-latency: slow disk, healthy database =="
cleanup_scenario "$SCENARIO"
mkdir -p "$(scenario_dir "$SCENARIO")"
DIR="$(scenario_dir "$SCENARIO")"

# A real, idle instance — no induced load — so classify_db has real
# evidence to call healthy, not just "no DB configured."
start_postgres "$SCENARIO"

echo "  building a ${SCRATCH_MB}MB scratch file for I/O contention..."
dd if=/dev/zero of="$DIR/scratch" bs=1M count="$SCRATCH_MB" status=none
sync

DEVICE="$(findmnt -no SOURCE --target "$DIR" 2>/dev/null || echo unknown)"
echo "  scratch file backed by: $DEVICE"

start_service "$SCENARIO" "$DEMO_PG_SOCKET_DIR" "$DEMO_PG_PORT"
launch_console "$SCENARIO"

echo "  starting $READERS real O_DIRECT reader(s), $PASSES_PER_READER" \
  "full sequential pass(es) of the scratch file each..."
for i in $(seq 1 "$READERS"); do
  (
    for _pass in $(seq 1 "$PASSES_PER_READER"); do
      ionice -c2 -n7 dd if="$DIR/scratch" of=/dev/null bs=4k iflag=direct \
        status=none 2>/dev/null
    done
  ) &
  track_pid "$SCENARIO" "reader-$i" "$!"
done

echo "  sustaining contention for ${CONTENTION_SECONDS}s so it covers at" \
  "least 3 persisted samples..."
sleep "$CONTENTION_SECONDS"

echo "  checking the correlation verdict..."
set +e
check_verdict "$SCENARIO" 20
VERDICT_STATUS=$?
set -e

echo "elapsed: $((SECONDS - START))s"
if [ "$VERDICT_STATUS" -ne 0 ]; then
  echo
  echo "This scenario is the least deterministic of the three — see" \
    "docs/demo/RUNBOOK.md's storage-latency troubleshooting section" \
    "before assuming a correlation-engine defect. Fast virtualized/SSD" \
    "storage may not queue enough under unprivileged dd contention to" \
    "cross the real thresholds; check the io_latency_ms evidence value" \
    "printed above against 30ms (HIGH) / 80ms (CRITICAL)."
fi
echo "service + console are still running — open the Correlation tab."
echo "run scripts/demo/reset.sh when done."
exit "$VERDICT_STATUS"
