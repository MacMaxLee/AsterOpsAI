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
/// §5, SRS FR-CAP-002). Every family starts `Unavailable` in unit U0 since no
/// telemetry, DBMS, or action logic exists yet; later units populate real
/// values as each capability is implemented.
pub fn default_capability_registry() -> BTreeMap<CapabilityFamily, Capability> {
    use CapabilityFamily::*;
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
        (
            family,
            Capability::Unavailable {
                reason: "not implemented until a later unit".to_string(),
            },
        )
    })
    .collect()
}
