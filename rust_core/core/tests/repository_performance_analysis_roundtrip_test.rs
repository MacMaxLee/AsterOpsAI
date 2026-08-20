//! Write-then-read-back coverage for the `performance_analysis` table
//! (migrations/V5, unit U5). Integration tests are already test-only code;
//! the workspace's unwrap/expect deny targets production code paths, not
//! `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod common;

use ai_ops_core::repository::{self, reader, PerformanceAnalysisRow};
use chrono::Utc;
use common::TestRepo;

#[tokio::test]
async fn writes_and_reads_back_a_performance_analysis_row() {
    let repo = TestRepo::open();

    let row = PerformanceAnalysisRow {
        ts: Utc::now(),
        subject: "HOST".to_string(),
        score: 87.0,
        score_version: "host-v1".to_string(),
        verdict: "CPU".to_string(),
        evidence_json: r#"[{"metric":"cpu_pressure_sustained_fraction","observed":0.8}]"#
            .to_string(),
    };

    repository::try_persist_performance_analysis(&repo.handle, row.clone());

    // The writer is a single background thread fed by an mpsc channel
    // (ADR 0007) — poll briefly for the row to land rather than assuming
    // synchronous delivery.
    let mut seen = None;
    for _ in 0..50 {
        let conn = reader::checkout(&repo.handle.read_pool).expect("checkout");
        let mut stmt = conn
            .prepare("SELECT ts, subject, score, score_version, verdict, evidence_json FROM performance_analysis")
            .expect("prepare");
        let mut rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })
            .expect("query_map");
        if let Some(found) = rows.next() {
            seen = Some(found.expect("row"));
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let (_ts, subject, score, score_version, verdict, evidence_json) =
        seen.expect("performance_analysis row should have been written");
    assert_eq!(subject, "HOST");
    assert_eq!(score, 87.0);
    assert_eq!(score_version, "host-v1");
    assert_eq!(verdict, "CPU");
    assert_eq!(evidence_json, row.evidence_json);
}
