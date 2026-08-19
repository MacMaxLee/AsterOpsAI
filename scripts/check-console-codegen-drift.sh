#!/usr/bin/env bash
# CI guard: committed console/lib/generated/models/*.dart must match what
# tool/generate_models.dart actually emits from /schemas/*.schema.json.
# Mirrors check-schema-drift.sh's rust_core/contracts pattern.
set -euo pipefail
cd "$(dirname "$0")/../console"

dart run tool/generate_models.dart
dart format lib/generated/models
git diff --exit-code -- lib/generated/models/
