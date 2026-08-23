#!/usr/bin/env bash
# CI guard: two separate SRS/TRS requirements share the identical
# "no AI call reachable" shape, so one dependency-graph check covers both.
# - TRS §20 (unit U15 per TRS's own numbering): "No AI call is reachable
#   from this [correlation engine] code path — verified by a CI
#   dependency-graph check, not just by review."
# - SRS FR-PERF-005 (unit U64): "No component of host or database
#   scoring, or of bottleneck classification, may be produced or
#   influenced by an AI model. This is a hard boundary, not a default" —
#   i.e. the same claim, for `ai_ops_core::analysis` instead of
#   `ai_ops_core::correlation`.
# A plain textual grep for `ai::` inside either module's own files would
# miss a transitive dependency routed through an unrelated shared module,
# so this uses `cargo-modules` (compiler-grade name resolution via
# rust-analyzer, not text matching) to emit the real per-item dependency
# graph, then does a real graph reachability search (BFS over "uses"
# edges only — "owns" edges are just module-nesting structure, not a code
# dependency) from every item under `ai_ops_core::correlation` OR
# `ai_ops_core::analysis` to check whether any item under `ai_ops_core::ai`
# is reachable.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo-modules >/dev/null 2>&1; then
  echo "FAIL: cargo-modules is not installed. Run:" >&2
  echo "  cargo install cargo-modules --locked --version 0.27.0" >&2
  exit 1
fi

DOT=$(mktemp)
trap 'rm -f "$DOT"' EXIT

(cd rust_core && cargo modules dependencies -p core --lib --layout dot) >"$DOT" 2>/dev/null

if [ ! -s "$DOT" ]; then
  echo "FAIL: cargo modules dependencies produced no output" >&2
  exit 1
fi

# BFS over real "uses" edges (compiler-resolved, not grep) from every node
# under ai_ops_core::correlation or ai_ops_core::analysis; fails if any
# node under ai_ops_core::ai is reachable, directly or transitively.
violations=$(awk -F'"' '
/label="uses"/ {
  src = $2; dst = $4
  n = ++adjcount[src]
  adjtarget[src, n] = dst
  allnodes[src] = 1
  allnodes[dst] = 1
}
END {
  head = 1; tail = 0
  for (node in allnodes) {
    if ((node ~ /^ai_ops_core::correlation(::|$)/ || node ~ /^ai_ops_core::analysis(::|$)/) && !(node in visited)) {
      visited[node] = 1
      tail++
      queue[tail] = node
    }
  }
  while (head <= tail) {
    cur = queue[head]; head++
    cnt = adjcount[cur] + 0
    for (i = 1; i <= cnt; i++) {
      nxt = adjtarget[cur, i]
      if (!(nxt in visited)) {
        visited[nxt] = 1
        tail++
        queue[tail] = nxt
      }
    }
  }
  for (node in visited) {
    if (node ~ /^ai_ops_core::ai(::|$)/) {
      print node
    }
  }
}
' "$DOT")

if [ -n "$violations" ]; then
  echo "FORBIDDEN: the analysis/correlation code path reaches the AI module (TRS §20 / SRS FR-PERF-005):" >&2
  echo "$violations" | sort >&2
  exit 1
fi

echo "OK: no AI-module item is reachable from ai_ops_core::analysis or ai_ops_core::correlation (TRS §20 / SRS FR-PERF-005)"
