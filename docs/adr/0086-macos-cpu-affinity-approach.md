# 0086 — macOS CPU Affinity: Unsupported (Honest Capability Gap)

## Status

Accepted (unit U92).

## Context

Units U90–U91 brought macOS process control to parity with Linux for
self-process metrics and priority adjustment. CPU affinity is the third
action surface ADR 0013 shipped for Linux (via `sched_setaffinity(2)`), and
the question is whether/how to implement the same on macOS.

**macOS has no `sched_setaffinity` equivalent.** The platform offers three
alternatives, none of which provide the hard CPU-mask semantics Linux and
Windows both support:

1. **`thread_policy_set` with `THREAD_AFFINITY_POLICY`** — per-thread affinity
   *hint*, not a process-level guarantee. The scheduler may ignore it. This is
   explicitly documented as advisory ("the kernel will attempt to run the
   thread on the specified processor set, but may run it elsewhere").

2. **Quality of Service (QoS) classes** — `QOS_CLASS_USER_INTERACTIVE`,
   `QOS_CLASS_UTILITY`, etc. — are a different abstraction entirely (priority
   tiers + energy policy), not CPU index assignment. Mapping
   `CpuAffinityMask { cpus: BTreeSet<usize> }` to a QoS class loses the
   original request's meaning.

3. **Processor sets (`processor_set_*` APIs)** — deprecated since macOS 10.5,
   no longer functional on modern macOS. Documented as legacy.

Linux's `sched_setaffinity` and Windows' `SetProcessAffinityMask` both
guarantee: "this process only ever runs on these specific cores." macOS
provides no equivalent guarantee. The closest mechanism (`thread_policy_set`)
is per-thread, advisory, and requires enumerating/managing every thread in
the target process — a fundamentally different operation than the
process-level hard mask the `PlatformAdapter` trait expects.

## Decision

**macOS's `get_process_cpu_affinity` and `set_process_cpu_affinity` return
`CapabilityError::Unsupported` with a message referencing this ADR**, matching
the honest capability model (ADR 0004) rather than providing a misleading
best-effort implementation that cannot deliver the same semantics.

The returned error message:
```
"macOS lacks process-level hard CPU affinity masks; \
 see ADR 0086 for rationale (no sched_setaffinity equivalent)"
```

**Rationale for not implementing a best-effort `thread_policy_set` wrapper**:

1. **Semantic mismatch** — `PlatformAdapter::set_process_cpu_affinity`'s
   signature and name imply a process-level operation. Enumerating threads
   and setting per-thread hints is a different operation with different
   failure modes (new threads spawn without the hint, existing threads may
   ignore it).

2. **False guarantee** — Returning `Ok(())` from `set_process_cpu_affinity`
   implies the constraint was applied. On macOS, the scheduler may ignore the
   hint entirely. A caller has no way to verify the "affinity" actually took
   effect.

3. **Platform capability model precedent** — When a platform cannot support
   an operation's full contract, `Unsupported` is the honest answer (same as
   actions requiring privileges this process lacks, or operations impossible
   on the current kernel). A future unit targeting macOS-specific needs could
   add a separate, explicitly-advisory API if a real use case emerges.

**Test gating**: `core/tests/actions_host_priority_affinity_test.rs`'s
affinity tests are conditionally compiled `#[cfg(target_os = "linux")]` — they
assume `sched_setaffinity` semantics and would spuriously fail on macOS
anyway, since there's no macOS implementation to test.

## Consequences

- **`SetProcessCpuAffinityAction` is not registered on macOS.** A future
  wiring unit that registers action types must gate this one to
  `cfg!(target_os = "linux")` (Windows will eventually join once
  `SetProcessAffinityMask` is implemented there, ADR 0013 §Consequences).

- **macOS CPU affinity remains a known gap**, not a silent lie. If a real
  operator/user need for CPU placement control on macOS surfaces, that unit
  can:
  - Design a macOS-specific API surface (`set_thread_affinity_hint`?) that
    makes the advisory nature explicit, or
  - Map to QoS classes if energy/priority tiers are the actual need, or
  - Revisit this ADR with a concrete use case and accept the semantic
    mismatch if best-effort is acceptable for that scenario.

  Until then, `Unsupported` accurately describes the platform's capability.

- **Priority control (U91) and suspend/resume (future U93) are unaffected** —
  both have POSIX equivalents (`setpriority(2)`, `kill(2)` + SIGSTOP/SIGCONT)
  that work identically on macOS and Linux. Only affinity is an
  unsupportable-on-macOS outlier.

- **Test coverage**: Affinity tests are Linux-only; priority and
  suspend/resume tests run on both platforms (once U93 lands). The test suite
  accurately reflects which operations each OS supports.
