#!/usr/bin/env bash
# CI guard: SQL string literals may only live inside a DBMS adapter module,
# the SQLite repository layer, or core's own integration tests.
# core/src/dbms/adapters/ doesn't exist until unit U4 (a no-op exclusion
# until then); core/src/repository/ holds unit U2's SQLite
# migrations/queries; core/tests/ holds the repository layer's integration
# tests, which legitimately issue raw SQL to inspect or (in the
# tamper-detection test) deliberately bypass the repository layer to
# corrupt a row independent of its own query code.
set -euo pipefail
cd "$(dirname "$0")/.."

matches=$(grep -rniE '"[[:space:]]*(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP)[[:space:]]' \
  --include='*.rs' rust_core \
  | grep -Ev '^rust_core/core/src/(dbms/adapters|repository)/' \
  | grep -Ev '^rust_core/core/tests/' || true)

if [ -n "$matches" ]; then
  echo "FORBIDDEN: SQL string literal outside core/src/dbms/adapters/*, core/src/repository/*, or core/tests/*:" >&2
  echo "$matches" >&2
  exit 1
fi

echo "OK: no SQL literals outside core/src/dbms/adapters/*, core/src/repository/*, or core/tests/*"
