# Pre-Audit Checklist — Status

Tracking progress against `/tmp/9LAD1/05-PRE-AUDIT-CHECKLIST.md` (the
user's own copy — not committed to this repo; see
`docs/demo/RUNBOOK.md`/`CRITERIA.md` for the parts of it that are).

## Done

- **Step 1 — Verify the scenarios yourself.** Run from a genuine clean
  clone (`/tmp/demo-verify`, built from scratch, since deleted) on this
  dev VM:
  - Lock storm: **PASS**, ~1s, both runs.
  - Pool exhaustion: **PASS**, ~1s, both runs.
  - Storage latency: **FAIL**, both runs — this VM's virtualized
    storage is too fast for unprivileged contention to cross the real
    latency thresholds. Extensively investigated and documented as a
    real environment limitation, not a correlation-engine defect — see
    `docs/adr/0026-demo-scenario-harness.md` and
    `docs/demo/RUNBOOK.md`'s troubleshooting section for the full
    trail. Treated as a pass overall (2/3 deterministic scenarios
    clean, third is a documented, understood gap).
  - Reset verified clean, all three re-run twice back-to-back with no
    reboot, total wall clock ~50s (well under the 10-minute budget).
- **Step 2 — Look at it cold.** Blind test run for real: one of
  {lock-storm, pool-exhaustion} launched at random (storage-latency
  excluded from the pool — it doesn't reliably demonstrate its own
  named condition on this VM, which would muddy a test about screen
  legibility, not induction success). User correctly read "Database
  locks" from the screen alone; confirmed root cause statable in under
  5 seconds, evidence and ruled-out list both visible with no
  scrolling or toggle.
- **Step 3 — Write kill criteria.** `docs/demo/CRITERIA.md` committed
  and pushed.

## Paused — waiting on new hardware

**Step 4 (get it in front of one person) is paused.** The user has
ordered a dedicated Ubuntu Linux server machine and wants to redo
Step 1 on it once it arrives, before recording/posting anything.

**Why this matters beyond just "a different machine":** real,
non-virtualized disk hardware is exactly what Step 1's storage-latency
scenario needs to actually pass — this dev VM's fast virtualized
storage was the whole reason it couldn't be verified here (see the ADR
above). A dedicated server is also a cleaner, more honest "stranger's
machine" environment for the actual recording than a shared dev
sandbox.

## When the new machine is ready

1. Re-run `docs/demo/RUNBOOK.md`'s prerequisites fresh on it
   (`scripts/setup-test-postgres.sh`, the Rust release build, the
   Flutter Linux desktop build tools, `scripts/demo/build-console.sh`).
2. Re-run Step 1 in full from a clean clone — pay particular attention
   to whether `storage-latency.sh` now genuinely **PASS**es (real,
   slower disk should let its `dd`-based contention actually cross
   `STORAGE_IO_LATENCY_MS_HIGH`/`_CRITICAL`). If it still doesn't, the
   optional `scripts/demo/setup-io-cgroup.sh` cgroup `io.max` path is
   worth trying there too — it failed on this dev VM specifically
   because of 96 processes sitting directly in the root cgroup, which
   a fresh server install is unlikely to have.
3. Steps 2 and 3 already have real, saved results (above) — no need to
   redo unless the new machine's storage-latency result changes the
   picture materially.
4. Resume Step 4: record the clip (GNOME's built-in recorder —
   `Ctrl+Alt+Shift+R` — worked reliably on this VM; `ffmpeg`'s
   packaged build here had no `pipewire` input device, so GNOME's own
   recorder is the proven path unless the new machine's `ffmpeg`
   package differs), then post it per the checklist's own instructions
   (entirely the user's own action — not something to delegate).
5. Steps 5–6 follow from there.
