#!/usr/bin/env bash
# CI guard: SQL string literals may only live inside a DBMS adapter module.
# core/src/dbms/adapters/ doesn't exist until unit U4 — until then this is a
# guaranteed no-op, and needs zero changes when U4 lands.
set -euo pipefail
cd "$(dirname "$0")/.."

matches=$(grep -rniE '"[[:space:]]*(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP)[[:space:]]' \
  --include='*.rs' rust_core \
  | grep -Ev '^rust_core/core/src/dbms/adapters/' || true)

if [ -n "$matches" ]; then
  echo "FORBIDDEN: SQL string literal outside core/src/dbms/adapters/*:" >&2
  echo "$matches" >&2
  exit 1
fi

echo "OK: no SQL literals outside core/src/dbms/adapters/*"
