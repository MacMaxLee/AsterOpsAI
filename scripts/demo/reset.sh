#!/usr/bin/env bash
# Returns everything the U21 demo scenarios might have left running back
# to a clean state: kills exactly the PIDs each scenario tracked
# (postgres/service/console) and removes the demo's own scratch
# directories. Never a broad pkill of postgres/service/console
# system-wide — only what this demo itself started and tracked under
# demo/.state/.
set -euo pipefail

# shellcheck source=lib/common.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

SCENARIOS=(lock-storm pool-exhaustion storage-latency)

for scenario in "${SCENARIOS[@]}"; do
  dir="$(scenario_dir "$scenario")"
  if [ -d "$dir" ]; then
    echo "resetting '$scenario'..."
    cleanup_scenario "$scenario"
  fi
done

if [ -d "$STATE_ROOT" ] && [ -z "$(ls -A "$STATE_ROOT" 2>/dev/null)" ]; then
  rmdir "$STATE_ROOT"
fi

echo "reset complete."
