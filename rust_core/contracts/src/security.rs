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
}
