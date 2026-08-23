//! SRS FR-HIST-002, narrowed (unit U62, see docs/adr/0067): a benchmark
//! run's own substantive results are immutable once written — only its
//! lifecycle status (`rolled_back`/`rollback_action_id`) may transition,
//! and only once, via a real compare-and-swap guard
//! (`repository::benchmark::mark_rolled_back`) mirroring
//! `policy::transition`'s own CAS pattern for `actions` rows.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::repository::{
    self, get_benchmark_run, insert_benchmark_run, mark_benchmark_run_rolled_back, NewBenchmarkRun,
    NewProposedAction, RepositoryConfig, RepositoryError, RepositoryHandle,
};
use chrono::Utc;
use repo_common::TestRepo;

/// `rollback_action_id` carries a real `FOREIGN KEY REFERENCES actions
/// (id)` (migrations/V10) — a plain, minimal `actions` row via
/// `repository::propose_action` directly (not the full policy-engine
/// validate/evaluate path `policy_approval_lifecycle_test.rs`'s own
/// `propose_and_grant` helper uses, which is irrelevant to what this
/// test proves) is all that's needed to satisfy it.
async fn a_real_action_id(handle: &RepositoryHandle, now: chrono::DateTime<Utc>, pid: i64) -> i64 {
    let row = repository::propose_action(
        handle,
        NewProposedAction {
            created_at: now,
            action_type: "test.rollback_target".to_string(),
            // `pid` varies per call so two calls in the same test never
            // collide with `has_unresolved_action`'s own real
            // "conflicting action already in flight" check (policy.rs)
            // — each is a genuinely distinct target, not a real retry.
            target_identity_json: format!(
                r#"{{"kind":"PROCESS","pid":{pid},"start_time_ticks":1}}"#
            ),
            target_start_time: 1,
            risk_classification: "INFORMATIONAL".to_string(),
            status: "AUTO_ALLOWED".to_string(),
            evidence_json: "{}".to_string(),
            requested_by: "tester".to_string(),
            parameters_json: "{}".to_string(),
            parameters_hash: "deadbeef".to_string(),
            resource_descriptor_json: r#"{"kind":"PROCESS","name":"dummy"}"#.to_string(),
            approval_expires_at: None,
            rollback_of: None,
        },
    )
    .await
    .expect("propose_action");
    row.id
}

fn new_run(now: chrono::DateTime<Utc>) -> NewBenchmarkRun {
    NewBenchmarkRun {
        created_at: now,
        // `action_id` has a real FOREIGN KEY constraint against `actions`
        // — `None` avoids needing an unrelated real `actions` row just to
        // satisfy it; the substantive-fields/rollback-guard behavior
        // this test proves doesn't depend on a real action linkage.
        action_id: None,
        metric_name: "cpu_utilization_percent".to_string(),
        direction: "LOWER_IS_BETTER".to_string(),
        baseline_window_start: now,
        baseline_window_end: now,
        baseline_cv: Some(0.05),
        post_change_window_start: Some(now),
        post_change_window_end: Some(now),
        post_change_cv: Some(0.09),
        p_value: Some(0.01),
        verdict: "REGRESSED".to_string(),
        confounders_json: "[]".to_string(),
    }
}

#[tokio::test]
async fn rolling_back_a_run_never_touches_its_substantive_results() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let inserted = insert_benchmark_run(&repo.handle, new_run(now))
        .await
        .expect("insert_benchmark_run");
    assert!(!inserted.rolled_back);
    assert_eq!(inserted.rollback_action_id, None);

    let rollback_action_id = a_real_action_id(&repo.handle, now, 1).await;
    let rolled_back = mark_benchmark_run_rolled_back(&repo.handle, inserted.id, rollback_action_id)
        .await
        .expect("mark_benchmark_run_rolled_back");

    // Lifecycle status genuinely transitioned.
    assert!(rolled_back.rolled_back);
    assert_eq!(rolled_back.rollback_action_id, Some(rollback_action_id));

    // Every substantive result field is byte-for-byte unchanged.
    assert_eq!(rolled_back.id, inserted.id);
    assert_eq!(rolled_back.created_at, inserted.created_at);
    assert_eq!(rolled_back.action_id, inserted.action_id);
    assert_eq!(rolled_back.metric_name, inserted.metric_name);
    assert_eq!(rolled_back.direction, inserted.direction);
    assert_eq!(
        rolled_back.baseline_window_start,
        inserted.baseline_window_start
    );
    assert_eq!(
        rolled_back.baseline_window_end,
        inserted.baseline_window_end
    );
    assert_eq!(rolled_back.baseline_cv, inserted.baseline_cv);
    assert_eq!(
        rolled_back.post_change_window_start,
        inserted.post_change_window_start
    );
    assert_eq!(
        rolled_back.post_change_window_end,
        inserted.post_change_window_end
    );
    assert_eq!(rolled_back.post_change_cv, inserted.post_change_cv);
    assert_eq!(rolled_back.p_value, inserted.p_value);
    assert_eq!(rolled_back.verdict, inserted.verdict);
    assert_eq!(rolled_back.confounders_json, inserted.confounders_json);
}

#[tokio::test]
async fn a_second_rollback_is_rejected_not_silently_overwritten() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let inserted = insert_benchmark_run(&repo.handle, new_run(now))
        .await
        .expect("insert_benchmark_run");

    let first_rollback_id = a_real_action_id(&repo.handle, now, 1).await;
    mark_benchmark_run_rolled_back(&repo.handle, inserted.id, first_rollback_id)
        .await
        .expect("first mark_benchmark_run_rolled_back");

    let second_rollback_id = a_real_action_id(&repo.handle, now, 2).await;
    let second =
        mark_benchmark_run_rolled_back(&repo.handle, inserted.id, second_rollback_id).await;
    match second {
        Err(RepositoryError::BenchmarkRunAlreadyRolledBack(id)) => {
            assert_eq!(id, inserted.id);
        }
        other => panic!("expected BenchmarkRunAlreadyRolledBack, got {other:?}"),
    }

    // The rejected second call must be a true no-op: the row still
    // shows the *first* rollback_action_id, not silently overwritten
    // with the second, rejected attempt's value.
    let current = get_benchmark_run(&repo.handle, inserted.id)
        .await
        .expect("get_benchmark_run")
        .expect("row exists");
    assert_eq!(current.rollback_action_id, Some(first_rollback_id));
}

#[tokio::test]
async fn rolling_back_a_nonexistent_run_reports_not_found_not_already_rolled_back() {
    let repo = TestRepo::open();

    let result = mark_benchmark_run_rolled_back(&repo.handle, 999_999, 1).await;
    match result {
        Err(RepositoryError::PolicyActionNotFound(id)) => {
            assert_eq!(id, 999_999);
        }
        other => {
            panic!("expected PolicyActionNotFound (the reused not-found error), got {other:?}")
        }
    }
}

/// Durability across a fresh connection, the same "a real second service
/// lifetime, not the same in-process handle" proof every `known_*`
/// detector test in this session already establishes.
#[tokio::test]
async fn the_rollback_transition_is_durable_across_a_fresh_connection() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let inserted = insert_benchmark_run(&repo.handle, new_run(now))
        .await
        .expect("insert_benchmark_run");
    let rollback_action_id = a_real_action_id(&repo.handle, now, 1).await;
    mark_benchmark_run_rolled_back(&repo.handle, inserted.id, rollback_action_id)
        .await
        .expect("mark_benchmark_run_rolled_back");

    let db_path = repo.db_path().to_path_buf();
    let second_handle =
        repository::init(&RepositoryConfig { db_path }).expect("re-open the same database file");

    let reread = get_benchmark_run(&second_handle, inserted.id)
        .await
        .expect("get_benchmark_run")
        .expect("row exists");
    assert!(reread.rolled_back);
    assert_eq!(reread.rollback_action_id, Some(rollback_action_id));
}
