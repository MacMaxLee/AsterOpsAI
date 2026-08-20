#!/usr/bin/env bash
# Lock storm — blocking chain, healthy host (unit U21, REQUIREMENTS #1a).
# Single command: provisions a throwaway PostgreSQL, holds a real
# row-level lock via one session, blocks six real waiter sessions on it,
# starts service + the console pointed at it, and checks the resulting
# correlation verdict against demo/scenarios/lock-storm/expected-
# verdict.json. Re-runnable: tears its own prior run down first.
set -euo pipefail

# shellcheck source=lib/common.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

SCENARIO="lock-storm"
START=$SECONDS

echo "== lock-storm: blocking chain, healthy host =="
cleanup_scenario "$SCENARIO"
mkdir -p "$(scenario_dir "$SCENARIO")"

start_postgres "$SCENARIO"

echo "  seeding the locked table..."
demo_psql "$SCENARIO" -c "CREATE TABLE t (id int); INSERT INTO t VALUES (1);" >/dev/null

start_service "$SCENARIO" "$DEMO_PG_SOCKET_DIR" "$DEMO_PG_PORT"
launch_console "$SCENARIO"

echo "  acquiring the real lock holder..."
DIR="$(scenario_dir "$SCENARIO")"
HOLDER_FIFO="$DIR/holder.fifo"
mkfifo "$HOLDER_FIFO"
exec 3<>"$HOLDER_FIFO"
demo_psql_argv "$SCENARIO"
( exec "${DEMO_PSQL_ARGV[@]}" <&3 ) >"$DIR/holder.log" 2>&1 &
track_pid "$SCENARIO" holder "$!"
echo "BEGIN;" >&3
echo "SELECT id FROM t WHERE id = 1 FOR UPDATE;" >&3
sleep 0.5

echo "  starting six real waiter sessions..."
for i in $(seq 1 6); do
  demo_psql_argv "$SCENARIO"
  ( exec "${DEMO_PSQL_ARGV[@]}" -c "SELECT id FROM t WHERE id = 1 FOR UPDATE" ) \
    >"$DIR/waiter-$i.log" 2>&1 &
  track_pid "$SCENARIO" "waiter-$i" "$!"
done

echo "  waiting for the block to register in pg_stat_activity..."
DEADLINE=$((SECONDS + 10))
while true; do
  WAITING="$(demo_psql "$SCENARIO" -tA -c \
    "SELECT count(*) FROM pg_stat_activity WHERE wait_event_type = 'Lock'")"
  if [ "$WAITING" -ge 6 ]; then
    break
  fi
  if [ "$SECONDS" -ge "$DEADLINE" ]; then
    echo "error: the six waiters never registered as blocked" >&2
    exit 1
  fi
  sleep 0.2
done

echo "  checking the correlation verdict..."
check_verdict "$SCENARIO" 30

echo "elapsed: $((SECONDS - START))s"
echo "service + console are still running — open the Correlation tab."
echo "run scripts/demo/reset.sh when done."
