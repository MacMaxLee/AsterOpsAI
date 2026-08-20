#!/usr/bin/env bash
# Connection pool exhaustion — DB idle, app starved (unit U21,
# REQUIREMENTS #1b). Single command: provisions a throwaway PostgreSQL
# with a small max_connections, opens real idle client connections
# (connect, run one query, then sit genuinely idle — not busy-looping)
# until the session/max_connections ratio crosses the real
# DB_CONN_SATURATION_HIGH threshold, starts service + the console, and
# checks the verdict against demo/scenarios/pool-exhaustion/expected-
# verdict.json. Re-runnable: tears its own prior run down first.
set -euo pipefail

# shellcheck source=lib/common.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

SCENARIO="pool-exhaustion"
START=$SECONDS

# max_connections=20 with 5 real background workers already using slots
# on a fresh instance (checkpointer, background writer, walwriter,
# autovacuum launcher, logical replication launcher — confirmed
# empirically, not guessed): 12 real idle client connections lands the
# ratio at 18/20 = 0.90, comfortably between HIGH (0.85) and CRITICAL
# (0.95).
MAX_CONNECTIONS=20
IDLE_CONNECTIONS=12

echo "== pool-exhaustion: DB idle, app starved =="
cleanup_scenario "$SCENARIO"
mkdir -p "$(scenario_dir "$SCENARIO")"

start_postgres "$SCENARIO" -c "max_connections=$MAX_CONNECTIONS"

start_service "$SCENARIO" "$DEMO_PG_SOCKET_DIR" "$DEMO_PG_PORT"
launch_console "$SCENARIO"

echo "  opening $IDLE_CONNECTIONS real idle connections..."
DIR="$(scenario_dir "$SCENARIO")"
for i in $(seq 1 "$IDLE_CONNECTIONS"); do
  fifo="$DIR/idle-$i.fifo"
  mkfifo "$fifo"
  # A distinct high fd per connection (100+i) so each has its own
  # independent read-write file description on its own fifo.
  eval "exec $((100 + i))<>\"\$fifo\""
  demo_psql_argv "$SCENARIO"
  ( exec "${DEMO_PSQL_ARGV[@]}" <&"$((100 + i))" ) >"$DIR/idle-$i.log" 2>&1 &
  track_pid "$SCENARIO" "idle-$i" "$!"
  # One real query so the session genuinely exists and is visible, then
  # nothing further — the fifo staying open (never closed, never fed
  # EOF) is what keeps the connection sitting idle rather than exiting.
  eval "echo 'SELECT 1;' >&$((100 + i))"
done

echo "  waiting for the real session count to cross saturation..."
DEADLINE=$((SECONDS + 10))
THRESHOLD=$(python3 -c "print(int($MAX_CONNECTIONS * 0.85))")
while true; do
  COUNT="$(demo_psql "$SCENARIO" -tA -c "SELECT count(*) FROM pg_stat_activity")"
  if [ "$COUNT" -ge "$THRESHOLD" ]; then
    break
  fi
  if [ "$SECONDS" -ge "$DEADLINE" ]; then
    echo "error: session count never crossed saturation ($COUNT/$MAX_CONNECTIONS)" >&2
    exit 1
  fi
  sleep 0.2
done

echo "  checking the correlation verdict..."
check_verdict "$SCENARIO" 30

echo "elapsed: $((SECONDS - START))s"
echo "service + console are still running — open the Correlation tab."
echo "run scripts/demo/reset.sh when done."
