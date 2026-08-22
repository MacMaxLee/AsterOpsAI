#!/usr/bin/env bash
# CI guard (SRS §36, TRS §45, unit U61): the committed docs/traceability-
# matrix.md must match what generate-traceability-matrix.sh actually
# produces (drift check, mirroring check-schema-drift.sh's own pattern),
# and no un-allowlisted FR-ID may be MISSING (zero references anywhere in
# the repo) — SRS §36's own wording: "An FR-ID with no test is a release
# blocker, not a documentation gap."
#
# The only allowed MISSING FR-IDs are FR-FLEET-*: the fleet coordinator is
# explicitly v2 (CLAUDE.md, SRS §35) and doesn't exist in this repo yet, so
# these are a real, deliberate, named exception — not a gap this gate
# should ever block a v1 tag over. Any other MISSING FR-ID fails the gate.
set -euo pipefail
cd "$(dirname "$0")/.."

ALLOWLIST_MISSING="FR-FLEET-001 FR-FLEET-002 FR-FLEET-003"

TMP_OUT=$(mktemp)
trap 'rm -f "$TMP_OUT"' EXIT

cp docs/traceability-matrix.md "$TMP_OUT.committed" 2>/dev/null || true
bash scripts/generate-traceability-matrix.sh >/dev/null
if ! diff -q "$TMP_OUT.committed" docs/traceability-matrix.md >/dev/null 2>&1; then
  echo "FAIL: docs/traceability-matrix.md is stale — run" >&2
  echo "  bash scripts/generate-traceability-matrix.sh" >&2
  echo "and commit the result." >&2
  rm -f "$TMP_OUT.committed"
  exit 1
fi
rm -f "$TMP_OUT.committed"

# -a: the generated matrix embeds SRS.md's own prose verbatim (real
# multi-byte punctuation included), which some environments' `grep`
# otherwise misdetects as binary and silently returns zero matches for.
missing_rows=$(grep -a '| MISSING |' docs/traceability-matrix.md || true)
real_gaps=""
while IFS= read -r row; do
  [ -z "$row" ] && continue
  fr=$(echo "$row" | grep -oE 'FR-[A-Z]+-[0-9]+' | head -1)
  if ! echo " $ALLOWLIST_MISSING " | grep -q " $fr "; then
    real_gaps="$real_gaps
$row"
  fi
done <<<"$missing_rows"

if [ -n "$real_gaps" ]; then
  echo "FORBIDDEN: FR-ID(s) with zero traceability anywhere in the repo (not on the v2 allowlist):" >&2
  echo "$real_gaps" >&2
  exit 1
fi

echo "OK: docs/traceability-matrix.md is current, no un-allowlisted MISSING FR-IDs"
