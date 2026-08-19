#!/usr/bin/env bash
# Idempotent: downloads and extracts real PostgreSQL 13/15/17 server +
# client binaries from the official PGDG apt repository (apt.postgresql.org)
# directly over HTTPS, with `dpkg -x` (no `dpkg -i`, no apt, no root, no
# system-wide install, no Docker) into ~/.local-tools/pgNN/. Used by
# core/tests/dbms/common/mod.rs to spin up real PostgreSQL server instances
# for integration tests in environments without Docker — see the U4 plan's
# "Environment realities" section for why, and this script's own steps are
# exactly what was hand-verified working before being scripted here.
#
# Safe to re-run: skips any version whose binaries already exist.
set -euo pipefail

ARCH=$(dpkg --print-architecture)
BASE_URL="https://apt.postgresql.org/pub/repos/apt/"
INDEX_URL="${BASE_URL}dists/noble-pgdg/main/binary-${ARCH}/Packages"
DEST_ROOT="${PG_TEST_INSTALL_ROOT:-$HOME/.local-tools}"
SHARED_DEST="$DEST_ROOT/pg-shared"
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

PACKAGES_FILE="$WORK_DIR/Packages"
curl -sf "$INDEX_URL" -o "$PACKAGES_FILE"

# Prints the `Filename:` of the first (== newest, PGDG lists newest-first
# per package name) stanza for the given package name.
filename_for() {
  local pkg="$1"
  awk -v pkg="$pkg" '
    /^Package: / { in_pkg = ($2 == pkg) }
    in_pkg && /^Filename: / { print $2; exit }
  ' "$PACKAGES_FILE"
}

download_and_extract() {
  local pkg="$1" dest="$2"
  local filename
  filename=$(filename_for "$pkg")
  if [ -z "$filename" ]; then
    echo "error: package '$pkg' not found in $INDEX_URL" >&2
    exit 1
  fi
  local deb="$WORK_DIR/$(basename "$filename")"
  echo "  fetching $pkg ($(basename "$filename"))"
  curl -sf "${BASE_URL}${filename}" -o "$deb"
  dpkg -x "$deb" "$dest"
}

# libpq.so.5 is a separate package (`libpq5`), shared across every server
# major version's client tools — extracted once, reused by all of them.
# Located by `find` rather than a hardcoded multiarch directory name (dpkg's
# own arch name, e.g. "arm64", is NOT the multiarch triplet its libraries
# actually get installed under, e.g. "aarch64-linux-gnu" — don't derive one
# from the other by string surgery).
setup_libpq() {
  if find "$SHARED_DEST" -iname 'libpq.so.5' 2>/dev/null | grep -q .; then
    echo "== libpq5 (shared) already present, skipping =="
    return
  fi
  mkdir -p "$SHARED_DEST"
  echo "== libpq5 (shared) =="
  download_and_extract libpq5 "$SHARED_DEST"
}

setup_version() {
  local version="$1"
  local dest="$DEST_ROOT/pg${version}"
  if [ -d "$dest/usr/lib/postgresql/${version}/bin" ]; then
    echo "== PostgreSQL $version already present at $dest, skipping =="
    return
  fi
  echo "== PostgreSQL $version =="
  mkdir -p "$dest"
  download_and_extract "postgresql-${version}" "$dest"
  download_and_extract "postgresql-client-${version}" "$dest"
  echo "  ok: $dest/usr/lib/postgresql/${version}/bin"
}

setup_libpq
for v in 13 15 17; do
  setup_version "$v"
done

LIBPQ_DIR=$(dirname "$(find "$SHARED_DEST" -iname 'libpq.so.5' | head -1)")
echo
echo "Done. core/tests/dbms/common/mod.rs picks these up automatically via"
echo "PG_TEST_INSTALL_ROOT (defaults to ~/.local-tools) — nothing further to"
echo "export by hand. For manual use:"
echo "  export LD_LIBRARY_PATH=\"$LIBPQ_DIR\""
