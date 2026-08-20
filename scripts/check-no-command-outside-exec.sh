#!/usr/bin/env bash
# CI guard: std::process::Command may only be called from platform/*/exec.rs
# or */tests/ (unit U4's dbms integration test harness spawns/kills real
# throwaway PostgreSQL server processes for testing — see
# core/tests/dbms/common/mod.rs; unit U20's service/tests/support/mod.rs is
# the same harness, trimmed, for service's own correlation endpoint tests —
# a fundamentally different concern from the *product's own* runtime
# subprocess execution this gate exists to keep centralized/auditable;
# mirrors the same */tests/ exception already established for the
# SQL-outside-adapters gate).
set -euo pipefail
cd "$(dirname "$0")/.."

# Excludes comment/doc lines (`//...`) so mentioning the rule in prose
# doesn't trip the gate — only actual code usage counts.
matches=$(grep -rn 'std::process::Command' --include='*.rs' rust_core \
  | grep -Ev '^[^:]+:[0-9]+:\s*//' \
  | grep -Ev '^rust_core/platform/src/(linux|windows|macos)/exec\.rs:' \
  | grep -Ev '^rust_core/core/tests/' \
  | grep -Ev '^rust_core/service/tests/' || true)

if [ -n "$matches" ]; then
  echo "FORBIDDEN: std::process::Command used outside platform/*/exec.rs or */tests/:" >&2
  echo "$matches" >&2
  exit 1
fi

echo "OK: no std::process::Command usage outside platform/*/exec.rs or */tests/"
