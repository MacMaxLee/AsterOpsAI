use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A metric family's ability level. Every non-`Supported` variant carries a
/// human-readable reason — the server never reports a fabricated zero in
/// place of data it genuinely doesn't have (SRS FR-CAP-001).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Capability {
    Supported,
    Limited { reason: String },
    Unavailable { reason: String },
    PermissionRequired { reason: String },
}

/// Unit U33: the same four states `Capability` already has, with a
/// payload added to `Supported` — mirrors `core::dbms::capability::
/// Gated<T>` (`query_stats()`'s own return shape), the same
/// "parametric, tagged, payload-only-on-the-success-variant" pattern
/// `contracts::telemetry::MetricValue<T>` already establishes for
/// telemetry's own gated metrics, applied here to `Capability`'s set
/// instead.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GatedValue<T> {
    Supported { value: T },
    Limited { reason: String },
    Unavailable { reason: String },
    PermissionRequired { reason: String },
}

/// The families of capability this product can report on. Adding a family is
/// additive; removing or renaming one is a wire-breaking change.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityFamily {
    Cpu,
    Memory,
    Storage,
    Network,
    Process,
    Device,
    DbmsPostgresql,
    Security,
    Actions,
    SelfMetrics,
}

/// Single source of truth for what this build can currently report,
/// consumed identically by the server and by generated documentation (TRS
/// §5, SRS FR-CAP-002). Families implemented on the current OS start
/// `Supported`; everything else (DBMS, security, actions — later units — and
/// every family on a platform without a real implementation yet) starts
/// `Unavailable`.
pub fn default_capability_registry() -> BTreeMap<CapabilityFamily, Capability> {
    use CapabilityFamily::*;
    let implemented_on_this_platform: &[CapabilityFamily] = if cfg!(target_os = "linux") {
        &[Cpu, Memory, Storage, Network, Process, Device]
    } else {
        &[]
    };

    [
        Cpu,
        Memory,
        Storage,
        Network,
        Process,
        Device,
        DbmsPostgresql,
        Security,
        Actions,
        SelfMetrics,
    ]
    .into_iter()
    .map(|family| {
        let capability = if implemented_on_this_platform.contains(&family) {
            Capability::Supported
        } else {
            Capability::Unavailable {
                reason: "not implemented until a later unit".to_string(),
            }
        };
        (family, capability)
    })
    .collect()
}
