#!/usr/bin/env bash
# Builds the console's real Linux desktop release binary once, shared by
# all three demo scenario scripts (unit U21). Idempotent: skips the
# rebuild if a bundle already exists and is newer than pubspec.lock and
# every file under lib/.
#
# Requires Flutter's own Linux desktop prerequisites: clang, cmake,
# ninja-build, pkg-config, libgtk-3-dev (see docs/demo/RUNBOOK.md).
set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
CONSOLE_DIR="$REPO_ROOT/console"

if ! command -v flutter >/dev/null 2>&1; then
  echo "error: flutter is not on PATH" >&2
  exit 1
fi

existing_bundle="$(find "$CONSOLE_DIR/build/linux" -maxdepth 4 -type f -name console \
  -path '*/release/bundle/*' 2>/dev/null | head -1)"

needs_build=1
if [ -n "$existing_bundle" ]; then
  needs_build=0
  newest_source="$(find "$CONSOLE_DIR/lib" "$CONSOLE_DIR/pubspec.lock" -type f -newer "$existing_bundle" 2>/dev/null | head -1)"
  if [ -n "$newest_source" ]; then
    needs_build=1
  fi
fi

if [ "$needs_build" -eq 0 ]; then
  echo "console already built and up to date: $existing_bundle"
  exit 0
fi

echo "building console (release, one-time — this may take a minute)..."
(cd "$CONSOLE_DIR" && flutter build linux --release)

built_bundle="$(find "$CONSOLE_DIR/build/linux" -maxdepth 4 -type f -name console \
  -path '*/release/bundle/*' 2>/dev/null | head -1)"
if [ -z "$built_bundle" ]; then
  echo "error: build finished but no bundle was found under $CONSOLE_DIR/build/linux" >&2
  exit 1
fi
echo "built: $built_bundle"
