//! `security_events`/`security_incidents`/`security_suppressions`
//! (migrations/V6 + V12, unit U11). `insert_event` is the real
//! find-or-open-incident-then-insert-event sequence — safe as plain
//! sequential SQL (no `INSERT ... ON CONFLICT` needed) because it only
//! ever runs inside the single writer thread (ADR 0007), with no
//! concurrent writer that could interleave between the `SELECT` and the
//! `INSERT`s.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::error::RepositoryError;
use super::models::{
    NewSecurityEvent, NewSecuritySuppression, SecurityEventRow, SecurityIncidentRow,
};
use super::time::{format_ts, parse_ts};

const EVENT_COLUMNS: &str = "id, ts, detector_id, severity, category, summary, evidence_json, \
     incident_id, resource_descriptor_json";
const INCIDENT_COLUMNS: &str = "id, opened_at, closed_at, severity, status, summary";

fn event_from_sqlite(row: &rusqlite::Row) -> rusqlite::Result<SecurityEventRow> {
    Ok(SecurityEventRow {
        id: row.get(0)?,
        ts: parse_ts(&row.get::<_, String>(1)?),
        detector_id: row.get(2)?,
        severity: row.get(3)?,
        category: row.get(4)?,
        summary: row.get(5)?,
        evidence_json: row.get(6)?,
        incident_id: row.get(7)?,
        resource_descriptor_json: row.get(8)?,
    })
}

fn incident_from_sqlite(row: &rusqlite::Row) -> rusqlite::Result<SecurityIncidentRow> {
    Ok(SecurityIncidentRow {
        id: row.get(0)?,
        opened_at: parse_ts(&row.get::<_, String>(1)?),
        closed_at: row.get::<_, Option<String>>(2)?.map(|s| parse_ts(&s)),
        severity: row.get(3)?,
        status: row.get(4)?,
        summary: row.get(5)?,
    })
}

fn must_get_event(conn: &Connection, id: i64) -> Result<SecurityEventRow, RepositoryError> {
    let sql = format!("SELECT {EVENT_COLUMNS} FROM security_events WHERE id = ?1");
    conn.query_row(&sql, params![id], event_from_sqlite)
        .optional()?
        .ok_or(RepositoryError::SecurityEventNotFound(id))
}

fn must_get_incident(conn: &Connection, id: i64) -> Result<SecurityIncidentRow, RepositoryError> {
    let sql = format!("SELECT {INCIDENT_COLUMNS} FROM security_incidents WHERE id = ?1");
    conn.query_row(&sql, params![id], incident_from_sqlite)
        .optional()?
        .ok_or(RepositoryError::SecurityIncidentNotFound(id))
}

/// The real correlation step (SRS FR-SEC-002): reuses an already-`OPEN`
/// incident that has a prior event from the same `detector_id` against
/// the same resource, rather than opening a new one for every event —
/// "related events are correlated into a single incident rather than
/// reported as unrelated noise."
fn find_open_incident(
    conn: &Connection,
    detector_id: &str,
    resource_descriptor_json: &str,
) -> Result<Option<i64>, RepositoryError> {
    conn.query_row(
        "SELECT security_incidents.id FROM security_incidents
         JOIN security_events ON security_events.incident_id = security_incidents.id
         WHERE security_incidents.status = 'OPEN'
           AND security_events.detector_id = ?1
           AND security_events.resource_descriptor_json = ?2
         ORDER BY security_incidents.id DESC LIMIT 1",
        params![detector_id, resource_descriptor_json],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

/// Inserts a new security event, opening a fresh `OPEN` incident unless
/// [`find_open_incident`] finds one to reuse. Returns both rows — callers
/// (`core::security::incident::record_event`) need the incident id to
/// report where the event landed.
pub fn insert_event(
    conn: &Connection,
    new: &NewSecurityEvent,
) -> Result<(SecurityEventRow, SecurityIncidentRow), RepositoryError> {
    let incident_id =
        match find_open_incident(conn, &new.detector_id, &new.resource_descriptor_json)? {
            Some(id) => {
                escalate_incident_severity_if_higher(conn, id, &new.severity)?;
                id
            }
            None => {
                conn.execute(
                "INSERT INTO security_incidents (opened_at, closed_at, severity, status, summary) \
                 VALUES (?1, NULL, ?2, 'OPEN', ?3)",
                params![format_ts(new.ts), new.severity, new.summary],
            )?;
                conn.last_insert_rowid()
            }
        };

    conn.execute(
        "INSERT INTO security_events (
            ts, detector_id, severity, category, summary, evidence_json,
            incident_id, resource_descriptor_json
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            format_ts(new.ts),
            new.detector_id,
            new.severity,
            new.category,
            new.summary,
            new.evidence_json,
            incident_id,
            new.resource_descriptor_json,
        ],
    )?;
    let event = must_get_event(conn, conn.last_insert_rowid())?;
    let incident = must_get_incident(conn, incident_id)?;
    Ok((event, incident))
}

/// Unit U75 (ADR 0065's own "escalate severity" remainder): an
/// incident's severity used to be fixed forever at whichever event
/// happened to open it, so a later, more severe correlated event
/// (e.g. auth-failure repetition finally crossing the threshold) never
/// surfaced there. Raises the incident's stored severity to match a
/// newly correlated event's, but never lowers it — an incident that
/// was ever HIGH stays at least HIGH even if a later correlated event
/// is milder. An unparseable stored/new severity string is left alone
/// rather than guessed at; `security_incidents.severity`/`security_
/// events.severity` are both always written from `Severity::as_str`,
/// so this is a real-corruption guard, not an expected path.
fn escalate_incident_severity_if_higher(
    conn: &Connection,
    incident_id: i64,
    new_severity: &str,
) -> Result<(), RepositoryError> {
    let Some(new_rank) = crate::security::Severity::parse(new_severity) else {
        return Ok(());
    };
    let current: String = conn.query_row(
        "SELECT severity FROM security_incidents WHERE id = ?1",
        params![incident_id],
        |row| row.get(0),
    )?;
    let Some(current_rank) = crate::security::Severity::parse(&current) else {
        return Ok(());
    };
    if new_rank > current_rank {
        conn.execute(
            "UPDATE security_incidents SET severity = ?1 WHERE id = ?2",
            params![new_severity, incident_id],
        )?;
    }
    Ok(())
}

/// SRS FR-SEC-005: checks suppression first — a suppressed
/// (detector_id, resource) pair writes nothing at all, `None`. Otherwise
/// records the event via [`insert_event`], `Some`. Both steps run inside
/// the same call so the check and the write are atomic through the single
/// writer thread, same reasoning as [`insert_event`]'s own doc comment.
pub fn record_event_checked(
    conn: &Connection,
    new: &NewSecurityEvent,
) -> Result<Option<(SecurityEventRow, SecurityIncidentRow)>, RepositoryError> {
    if is_suppressed(conn, &new.detector_id, &new.resource_descriptor_json)? {
        return Ok(None);
    }
    insert_event(conn, new).map(Some)
}

/// SRS FR-SEC-003's own query: does `resource_descriptor_json` have any
/// event under a currently-`OPEN` incident? Checked at `actions::executor::
/// execute`'s freshest-possible point, same precedent as the
/// protected-resource stage (ADR 0012).
pub fn resource_is_flagged(
    conn: &Connection,
    resource_descriptor_json: &str,
) -> Result<bool, RepositoryError> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM security_incidents
            JOIN security_events ON security_events.incident_id = security_incidents.id
            WHERE security_incidents.status = 'OPEN'
              AND security_events.resource_descriptor_json = ?1
         )",
        params![resource_descriptor_json],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count != 0)
    .map_err(Into::into)
}

/// Unit U75 (ADR 0065's own named remainder): how many `detector_id`
/// events already exist for the exact same `resource_descriptor_json`
/// at or after `since` — `security::detect_auth_failure`'s own
/// severity-escalation input. Generic over `detector_id` rather than
/// hardcoded to auth-failure, matching every other function in this
/// module.
pub fn count_recent_events(
    conn: &Connection,
    detector_id: &str,
    resource_descriptor_json: &str,
    since: DateTime<Utc>,
) -> Result<i64, RepositoryError> {
    conn.query_row(
        "SELECT COUNT(*) FROM security_events \
         WHERE detector_id = ?1 AND resource_descriptor_json = ?2 AND ts >= ?3",
        params![detector_id, resource_descriptor_json, format_ts(since)],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

/// SRS FR-SEC-005: is `detector_id` suppressed, either everywhere
/// (`resource_descriptor_json IS NULL`) or specifically for this
/// resource?
pub fn is_suppressed(
    conn: &Connection,
    detector_id: &str,
    resource_descriptor_json: &str,
) -> Result<bool, RepositoryError> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM security_suppressions
            WHERE detector_id = ?1
              AND (resource_descriptor_json IS NULL OR resource_descriptor_json = ?2)
         )",
        params![detector_id, resource_descriptor_json],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count != 0)
    .map_err(Into::into)
}

/// Unit U15's security triage view: `OPEN` incidents, newest first —
/// same "read backward from now" reasoning `tuning::list_recent` already
/// used (ADR 0019), distinct from `policy::list_pending_approval`'s own
/// oldest-first inbox. Capped at a real, explicit limit (no retention
/// policy exists for `security_incidents`/`security_events` either).
/// `security_incidents` has no denormalized event count, so this is a
/// real `LEFT JOIN ... GROUP BY`, not a second query per row.
const OPEN_INCIDENTS_LIMIT: i64 = 100;

pub fn list_open_incidents(
    conn: &Connection,
) -> Result<Vec<(SecurityIncidentRow, i64)>, RepositoryError> {
    let sql = format!(
        "SELECT {cols}, COUNT(security_events.id) AS event_count
         FROM security_incidents
         LEFT JOIN security_events ON security_events.incident_id = security_incidents.id
         WHERE security_incidents.status = 'OPEN'
         GROUP BY security_incidents.id
         ORDER BY security_incidents.opened_at DESC
         LIMIT ?1",
        cols = INCIDENT_COLUMNS
            .split(", ")
            .map(|c| format!("security_incidents.{c}"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![OPEN_INCIDENTS_LIMIT], |row| {
            let incident = incident_from_sqlite(row)?;
            let event_count: i64 = row.get(6)?;
            Ok((incident, event_count))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn insert_suppression(
    conn: &Connection,
    new: &NewSecuritySuppression,
) -> Result<(), RepositoryError> {
    conn.execute(
        "INSERT INTO security_suppressions (
            created_at, created_by, detector_id, resource_descriptor_json, reason
        ) VALUES (?1,?2,?3,?4,?5)",
        params![
            format_ts(new.created_at),
            new.created_by,
            new.detector_id,
            new.resource_descriptor_json,
            new.reason,
        ],
    )?;
    Ok(())
}
