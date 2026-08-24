//! Wire types for the security triage view (unit U15).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecurityIncidentSummary {
    pub id: i64,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub severity: String,
    pub status: String,
    pub summary: String,
    /// How many detector events have correlated into this incident —
    /// `security_incidents` has no denormalized column for this; the
    /// server computes it via a real join, never a client-side guess.
    pub event_count: i64,
    /// Unit U78 (ADR 0023's own named gap, closed by docs/adr/0083): the
    /// `detector_id`/resource every event under this incident shares —
    /// FR-SEC-002's own correlation rule structurally guarantees this,
    /// since an incident only ever reuses an existing `OPEN` row for the
    /// exact same `(detector_id, resource)` pair. Present so a console
    /// per-incident suppress action has something real to suppress,
    /// unlike before this unit, when only an event *count* was visible.
    pub detector_id: String,
    pub resource_kind: String,
    pub resource_name: String,
}
