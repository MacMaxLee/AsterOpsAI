# Kill Criteria — Correlation Demo

Written before anyone outside this project sees the tool (pre-audit
checklist step 3), while still neutral. A number written beforehand is
the only defence against moving the goalposts after watching someone
struggle.

Sample size: whatever I get, minimum 1.

Question asked: **"what do you think is wrong with this database?"**

## Outcomes

**MAJORITY name the correct root cause unprompted**
→ Premise holds. Run Prompt B (the hardening audit), then plan the
next unit.

**SOME get it, some don't / general confusion**
→ Presentation problem, not a logic problem. The engine is provably
correct (checklist step 1 passed for lock-storm and pool-exhaustion,
the two deterministic scenarios — see `docs/demo/RUNBOOK.md` and
`docs/adr/0026-demo-scenario-harness.md` for storage-latency's own
documented, separate environment limitation). The next unit is a UI
unit, not a logic unit.

**NOBODY gets it, or nobody responds to the post at all**
→ Stop. Reconsider what the product is before writing more code.

## Decision reached

☐ premise holds ☐ presentation problem ☐ stop and reconsider

**Next unit is:** _______________________________________________
