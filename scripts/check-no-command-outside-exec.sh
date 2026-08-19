#!/usr/bin/env bash
# CI guard: std::process::Command may only be called from platform/*/exec.rs.
set -euo pipefail
cd "$(dirname "$0")/.."

# Excludes comment/doc lines (`//...`) so mentioning the rule in prose
# doesn't trip the gate — only actual code usage counts.
matches=$(grep -rn 'std::process::Command' --include='*.rs' rust_core \
  | grep -Ev '^[^:]+:[0-9]+:\s*//' \
  | grep -Ev '^rust_core/platform/src/(linux|windows|macos)/exec\.rs:' || true)

if [ -n "$matches" ]; then
  echo "FORBIDDEN: std::process::Command used outside platform/*/exec.rs:" >&2
  echo "$matches" >&2
  exit 1
fi

echo "OK: no std::process::Command usage outside platform/*/exec.rs"
