#!/usr/bin/env bash
# Regenerates /schemas/*.schema.json from rust_core/contracts. Commit the
# result in the same change as whatever contract edit prompted it.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo run -p contracts --bin emit-schemas
