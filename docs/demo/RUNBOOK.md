# AsterOpsAI Demo Runbook

Three self-contained scenarios that provision a real condition (a real
throwaway PostgreSQL instance, a real induced problem), start the real
service and console, and check the resulting cross-layer correlation
verdict against a written expected answer — so you know *before*
showing anyone whether the product is actually right. See
`docs/adr/0026-demo-scenario-harness.md` for the full design and every
real discovery made building this.

## Prerequisites

Run once, in order:

1. **PostgreSQL test binaries** (no Docker — extracted binaries, no
   root):
   ```
   scripts/setup-test-postgres.sh
   ```
2. **Rust release build** of the service (also happens automatically on
   first scenario run if skipped):
   ```
   cd rust_core && cargo build --release -p service && cd ..
   ```
3. **Console Linux desktop build.** Needs `clang cmake ninja-build
   pkg-config libgtk-3-dev` (`sudo apt-get install -y clang cmake
   ninja-build pkg-config libgtk-3-dev` if you don't have them), then:
   ```
   scripts/demo/build-console.sh
   ```
4. **(Optional, storage-latency only) cgroup `io.max` delegation.** Real
   dd-based contention alone may not be enough to cross the storage-
   latency thresholds on fast/virtualized storage (see the
   troubleshooting section below). If your machine's cgroup v2
   hierarchy has no stray processes sitting directly in the root
   cgroup, this can delegate genuine kernel-level I/O throttling to
   your user session — `storage-latency.sh` doesn't require it, but
   would use it if delegated:
   ```
   sudo scripts/demo/setup-io-cgroup.sh
   ```
5. **Offline mode.** `service` has no AI-provider wiring anywhere today
   — nothing in it calls out to the network, so no code here enforces
   "offline," there's simply nothing that needs to reach out. Stop
   Ollama and disable networking yourself if you want to demonstrate
   that explicitly (matches the pre-audit checklist's own step 1):
   ```
   sudo systemctl stop ollama
   sudo ip link set <iface> down
   ```
   Remember to bring networking back up afterward.

## The three commands

Each is a single command, provisions its own throwaway state, and is
safe to re-run without a reboot (it tears its own prior run down
first):

```
scripts/demo/lock-storm.sh        # blocking chain, healthy host
scripts/demo/pool-exhaustion.sh   # DB idle, app starved
scripts/demo/storage-latency.sh   # slow disk, healthy database
```

Each prints `PASS`/`FAIL` against its own
`demo/scenarios/<name>/expected-verdict.json`, leaves `service` and the
built console running (pointed at the same scenario, via a scoped
`XDG_RUNTIME_DIR`) so you can open the **Correlation** tab and see the
live verdict, and prints its own actual elapsed runtime.

## Reset

Returns everything back to a clean state — kills exactly the PIDs each
scenario tracked (postgres/service/console) and removes the demo's own
scratch directories. Never touches anything not started by this demo.

```
scripts/demo/reset.sh
```

## Expected output

**Lock storm** (real, verified — passes reliably, ~1-6s):
```
== lock-storm: blocking chain, healthy host ==
  ...
  checking the correlation verdict...
PASS (attempt 1)
{ "ranked": [{"cause": "DB_LOCKS", ...}, {"cause": "CLIENT_SIDE_APPLICATION", ...}],
  "ruled_out": [... 7 real health causes ...] }
elapsed: 1s
```

**Pool exhaustion** (real, verified — passes reliably, ~0-1s):
```
== pool-exhaustion: DB idle, app starved ==
  ...
PASS (attempt 1)
{ "ranked": [{"cause": "CONNECTION_EXHAUSTION", "confidence": 1.0, ...}, ...] }
elapsed: 0s
```
`DB_LOCKS` is deliberately *not* asserted ruled-out for this scenario
— see the note in its own `expected-verdict.json` and
`docs/adr/0026` for why (a real, pre-existing characteristic of
`classify_session_state`, unrelated to connection pooling itself).

**Storage latency** (real mechanism; **may FAIL on fast/virtualized
storage** — see below):
```
== storage-latency: slow disk, healthy database ==
  ...
FAIL after N attempt(s)
  - top ranked cause is 'HOST_CPU', expected 'STORAGE_LATENCY'
  ...
This scenario is the least deterministic of the three — see
docs/demo/RUNBOOK.md's storage-latency troubleshooting section...
```

## What to do when a scenario fails to reproduce

**Lock storm or pool exhaustion FAILs: stop.** These are deterministic,
real Postgres mechanics (a real row lock, a real connection count) that
have been verified to pass reliably. A FAIL here is a real
correlation-engine defect — fix it as its own unit before demoing
anything, per the pre-audit checklist's own step 1. Do not proceed to
show anyone a tool that gets the answer wrong.

**Storage latency FAILs: check the evidence before concluding a
defect.** This scenario depends on your machine's storage actually
being slow enough to queue under contention — confirmed, empirically,
NOT to be the case on the fast virtualized/SSD-backed storage this demo
was built on (`io_latency_ms` stayed around 0.03ms across every
technique tried — plain concurrent `dd`, `O_DIRECT`, `dsync` writes,
150 concurrent processes — all the way up to a genuine, `iowait`-
confirmed I/O-bound run; see `docs/adr/0026` for the full record).
Before treating a FAIL here as a correlation-engine problem:

1. Look at the printed verdict's `ranked[].evidence` for
   `storage_io_latency_sustained_fraction`, and check the underlying
   `io_latency_ms` value the host is actually reporting (via the
   Analysis tab, or `/api/v1/analysis/host`) against 30ms (`HIGH`) /
   80ms (`CRITICAL`). If it's nowhere near those, your storage is
   simply too fast for this scenario's unprivileged load generator —
   not a defect.
2. If the verdict instead ranks `HOST_CPU`, your machine's CPU is
   getting genuinely contended by the load-generation processes
   themselves (more likely on a 2-core box) — this is a real signal,
   correctly reported, just not the one this scenario means to
   demonstrate.
3. Try the optional `cgroup io.max` delegation (prerequisite 4 above)
   for genuine kernel-level throttling, if your machine's cgroup
   hierarchy supports it.
4. If none of that resolves it, this scenario's mechanism is real and
   correct — it just needs storage slow enough to actually queue. This
   is a known, documented limitation of the *demo scenario*, not of
   `core::correlation`/`core::analysis`, which are unmodified and were
   independently proven correct by the other two scenarios.
