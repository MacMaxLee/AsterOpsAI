#!/usr/bin/env bash
# CI guard: committed /schemas/*.schema.json must match what `contracts`
# actually emits. Run locally after any change to rust_core/contracts.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo run -p contracts --bin emit-schemas
git diff --exit-code -- schemas/
