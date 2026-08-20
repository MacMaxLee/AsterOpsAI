//! Profile -> `DesiredState` translation (SRS FR-TUNE-001). A small,
//! explicit, hand-picked table — not a formula — because there's no
//! principled way to derive "what BALANCED means" from first principles;
//! it's a documented judgment call (docs/adr/0015), same spirit as
//! `policy::risk::decide`'s own risk-level table.

use std::collections::BTreeSet;

use platform::{CpuAffinityMask, ProcessPriority};

/// What a profile (or a `Custom` override) wants for one target. `None` on
/// a field means "no opinion" — `candidates::build_candidates` proposes
/// nothing for it, rather than treating an absent preference as "reset to
/// some default."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredState {
    pub priority: Option<ProcessPriority>,
    pub cpu_affinity: Option<CpuAffinityMask>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuningProfile {
    Balanced,
    HighPerformance,
    BatterySaver,
    Development,
    Custom(DesiredState),
}

/// `std::thread::available_parallelism()` — a real, dependency-free "full
/// CPU set," with no new `PlatformAdapter` method needed. Falls back to a
/// single-CPU mask (never an empty one — an empty affinity mask would pin
/// a process to nothing) if the OS can't report it.
pub fn full_cpu_set() -> CpuAffinityMask {
    let n = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    CpuAffinityMask {
        cpus: (0..n).collect(),
    }
}

/// Pure. `Balanced`/`Development` both mean "OS default scheduling across
/// every CPU" — kept as two named variants rather than collapsed into one
/// because a future unit may give `Development` its own meaning (e.g.
/// disabling AUTO_LOW_RISK entirely) without disturbing `Balanced`.
/// `BatterySaver` pins to CPU 0 specifically — a documented, deliberately
/// simple choice (docs/adr/0015), not a claim that CPU 0 is always the
/// most power-efficient core on every real machine.
pub fn desired_state_for(profile: &TuningProfile, full_cpu_set: &CpuAffinityMask) -> DesiredState {
    match profile {
        TuningProfile::Balanced | TuningProfile::Development => DesiredState {
            priority: Some(ProcessPriority::Normal),
            cpu_affinity: Some(full_cpu_set.clone()),
        },
        TuningProfile::HighPerformance => DesiredState {
            priority: Some(ProcessPriority::AboveNormal),
            cpu_affinity: Some(full_cpu_set.clone()),
        },
        TuningProfile::BatterySaver => DesiredState {
            priority: Some(ProcessPriority::BelowNormal),
            cpu_affinity: Some(CpuAffinityMask {
                cpus: BTreeSet::from([0]),
            }),
        },
        TuningProfile::Custom(desired) => desired.clone(),
    }
}

/// The row's `profile` column value — `Custom`'s own desired state is
/// carried separately in `candidates_json`, not reconstructible from this
/// string alone (mirrors `ActionStatus`/`RiskLevel`'s own
/// string-round-trip precedent).
pub fn profile_label(profile: &TuningProfile) -> &'static str {
    match profile {
        TuningProfile::Balanced => "BALANCED",
        TuningProfile::HighPerformance => "HIGH_PERFORMANCE",
        TuningProfile::BatterySaver => "BATTERY_SAVER",
        TuningProfile::Development => "DEVELOPMENT",
        TuningProfile::Custom(_) => "CUSTOM",
    }
}
