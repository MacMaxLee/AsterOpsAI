//! Persists a [`DetectedEvent`], correlating it into an incident (SRS
//! FR-SEC-002) after a suppression check (FR-SEC-005) — the real
//! find-or-open-incident logic lives in `repository::security::
//! record_event_checked`, run atomically inside the single writer thread
//! (ADR 0007); this module is the typed entry point over it.

use crate::repository::{record_security_event, NewSecurityEvent, RepositoryHandle};

use super::detector::DetectedEvent;
use super::error::SecurityError;

#[derive(Debug, Clone)]
pub enum IncidentOutcome {
    /// The event was written and correlated into `incident_id` (a fresh
    /// incident, or an existing `OPEN` one from the same detector against
    /// the same resource).
    Recorded { event_id: i64, incident_id: i64 },
    /// FR-SEC-005: a suppression rule matched — nothing was written.
    Suppressed,
}

pub async fn record_event(
    handle: &RepositoryHandle,
    event: DetectedEvent,
) -> Result<IncidentOutcome, SecurityError> {
    let resource_descriptor_json = serde_json::to_string(&event.resource)?;
    let new = NewSecurityEvent {
        ts: event.ts,
        detector_id: event.detector_id.to_string(),
        severity: event.severity.as_str().to_string(),
        category: event.category.to_string(),
        summary: event.summary,
        evidence_json: event.evidence_json,
        resource_descriptor_json,
    };

    match record_security_event(handle, new).await? {
        Some((event_row, incident_row)) => Ok(IncidentOutcome::Recorded {
            event_id: event_row.id,
            incident_id: incident_row.id,
        }),
        None => Ok(IncidentOutcome::Suppressed),
    }
}
