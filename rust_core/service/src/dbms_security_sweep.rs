//! Unit U72 (ADR 0065/0077's own named follow-up, finally attempted):
//! the five best-effort DB-wide security/config checks
//! (`check_guc_changes`/`check_role_superuser_grants`/
//! `check_role_membership_grants`/`check_table_privilege_grants`/
//! `check_auth_failures`) used to piggyback on the `gucs` HTTP handler,
//! meaning detection only ever ran if *something* happened to poll
//! `/api/v1/dbms/gucs` — a headless deployment with no console attached
//! would never run any of these checks at all. This gives them their
//! own real, independent schedule, the same `tokio::spawn` +
//! `tokio::time::interval` shape `retention.rs` already establishes for
//! background work that must run regardless of HTTP traffic.

use std::sync::Arc;
use std::time::Duration;

use ai_ops_core::dbms::{log_tail, DbmsAdapter, GucValue as CoreGucValue};
use ai_ops_core::repository;
use ai_ops_core::security;
use chrono::Utc;

/// A real, explicit judgment call, not pinned by SRS/TRS (neither
/// specifies a detection-latency target for FR-DBSEC-001): frequent
/// enough that a superuser grant, a privilege change, or a burst of
/// auth failures is caught within well under a minute, infrequent
/// enough that five real DB round trips plus a log-file tail don't run
/// so often they meaningfully load the monitored instance. The
/// console's own former *accidental* cadence (whatever its configurable
/// 1-10s refresh interval happened to be, since detection rode along
/// with `gucs` polls) was never a deliberate choice for this — this
/// is one.
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// Only spawned when both a repository and a DBMS adapter are
/// configured (see `main.rs`) — a repository-less or DB-less service
/// has nothing to sweep or nowhere to record what it finds.
pub fn spawn(repo: repository::RepositoryHandle, adapter: Arc<dyn DbmsAdapter>) {
    spawn_with_interval(repo, adapter, SWEEP_INTERVAL);
}

/// Test-only override of the otherwise-fixed sweep interval — the same
/// `spawn_with_interval` precedent unit U62 established for
/// `self_metrics` and unit U65 for `HostTelemetrySampler`. Production
/// code always calls plain `spawn` above, never this directly.
pub fn spawn_with_interval(
    repo: repository::RepositoryHandle,
    adapter: Arc<dyn DbmsAdapter>,
    interval_duration: Duration,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(interval_duration);
        // First tick fires immediately; skip it so a freshly started
        // service doesn't sweep before the adapter's connection has
        // settled, mirroring `retention::spawn`'s own reasoning.
        interval.tick().await;
        loop {
            interval.tick().await;
            let gucs = match adapter.relevant_gucs().await {
                Ok(gucs) => gucs,
                Err(err) => {
                    tracing::warn!(error = %err, "relevant_gucs failed during scheduled security sweep");
                    continue;
                }
            };
            run_security_config_sweep(&repo, &adapter, &gucs).await;
        }
    });
}

/// Unit U55 (SRS FR-DBSEC-001(e)): the first production trigger for
/// `core::security`'s detection→incident pipeline. Best-effort: a
/// detection failure is logged, never blocks the rest of the sweep.
async fn check_guc_changes(repo: &repository::RepositoryHandle, gucs: &[CoreGucValue]) {
    let now = Utc::now();
    for guc in gucs {
        let previous = match repository::record_guc_value(repo, &guc.name, &guc.setting, now).await
        {
            Ok(previous) => previous,
            Err(err) => {
                tracing::warn!(error = %err, guc = %guc.name, "record_guc_value failed");
                continue;
            }
        };
        let Some(event) =
            security::detect_guc_change(&guc.name, previous.as_deref(), &guc.setting, now)
        else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, guc = %guc.name, "record_event failed for a detected GUC change");
        }
    }
}

/// Unit U56 (SRS FR-DBSEC-001(b), narrowed to the `rolsuper` flag —
/// see `security::detect_role_superuser_granted`'s own doc comment).
/// Same best-effort posture as `check_guc_changes`.
async fn check_role_superuser_grants(
    repo: &repository::RepositoryHandle,
    adapter: &Arc<dyn DbmsAdapter>,
) {
    let roles = match adapter.role_superuser_flags().await {
        Ok(roles) => roles,
        Err(err) => {
            tracing::warn!(error = %err, "role_superuser_flags failed");
            return;
        }
    };
    let now = Utc::now();
    for role in roles {
        let previous = match repository::record_role_superuser_flag(
            repo,
            &role.rolname,
            role.rolsuper,
            now,
        )
        .await
        {
            Ok(previous) => previous,
            Err(err) => {
                tracing::warn!(error = %err, rolname = %role.rolname, "record_role_superuser_flag failed");
                continue;
            }
        };
        let Some(event) =
            security::detect_role_superuser_granted(&role.rolname, previous, role.rolsuper, now)
        else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, rolname = %role.rolname, "record_event failed for a detected superuser grant");
        }
    }
}

/// Unit U58 (SRS FR-DBSEC-001(b)'s deferred remainder — see `security::
/// detect_role_membership_granted`'s own doc comment).
async fn check_role_membership_grants(
    repo: &repository::RepositoryHandle,
    adapter: &Arc<dyn DbmsAdapter>,
) {
    let memberships = match adapter.role_memberships().await {
        Ok(memberships) => memberships,
        Err(err) => {
            tracing::warn!(error = %err, "role_memberships failed");
            return;
        }
    };
    let now = Utc::now();
    for membership in memberships {
        let already_known = match repository::record_role_membership_seen(
            repo,
            &membership.member,
            &membership.granted_role,
            now,
        )
        .await
        {
            Ok(already_known) => already_known,
            Err(err) => {
                tracing::warn!(error = %err, member = %membership.member, granted_role = %membership.granted_role, "record_role_membership_seen failed");
                continue;
            }
        };
        let Some(event) = security::detect_role_membership_granted(
            &membership.member,
            &membership.granted_role,
            already_known,
            now,
        ) else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, member = %membership.member, "record_event failed for a detected role-membership grant");
        }
    }
}

/// Unit U59 (SRS FR-DBSEC-001(c), deliberately narrowed — see
/// `security::detect_table_privilege_granted`'s own doc comment).
async fn check_table_privilege_grants(
    repo: &repository::RepositoryHandle,
    adapter: &Arc<dyn DbmsAdapter>,
) {
    let grants = match adapter.table_privilege_grants().await {
        Ok(grants) => grants,
        Err(err) => {
            tracing::warn!(error = %err, "table_privilege_grants failed");
            return;
        }
    };
    let now = Utc::now();
    for grant in grants {
        let already_known = match repository::record_table_privilege_grant_seen(
            repo,
            &grant.grantee,
            &grant.schema,
            &grant.table,
            &grant.privilege_type,
            now,
        )
        .await
        {
            Ok(already_known) => already_known,
            Err(err) => {
                tracing::warn!(error = %err, grantee = %grant.grantee, table = %grant.table, "record_table_privilege_grant_seen failed");
                continue;
            }
        };
        let Some(event) = security::detect_table_privilege_granted(
            &grant.grantee,
            &grant.schema,
            &grant.table,
            &grant.privilege_type,
            already_known,
            now,
        ) else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, grantee = %grant.grantee, "record_event failed for a detected table privilege grant");
        }
    }
}

/// Unit U60 (SRS FR-DBSEC-001(a), the last FR-DBSEC-001 sub-item — see
/// `security::detect_auth_failure`'s own doc comment). Unlike the other
/// four checks, this does real local-host file I/O (`core::dbms::
/// log_tail`, deliberately not a `DbmsAdapter` method — see that
/// module's own doc comment) and simply never fires anything on a
/// non-co-located deployment (`csv_logging_enabled` false, or the log
/// directory unreadable/empty) — every other check keeps working
/// regardless.
async fn check_auth_failures(repo: &repository::RepositoryHandle, adapter: &Arc<dyn DbmsAdapter>) {
    let config = match adapter.auth_failure_log_config().await {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(error = %err, "auth_failure_log_config failed");
            return;
        }
    };
    if !config.csv_logging_enabled {
        return;
    }

    let active_file = match log_tail::find_active_csv_file(&config.log_dir).await {
        Ok(Some(path)) => path,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!(error = %err, log_dir = %config.log_dir.display(), "find_active_csv_file failed");
            return;
        }
    };
    let active_file_str = active_file.to_string_lossy().to_string();

    let previous_offset = match repository::get_log_tail_offset(repo, &active_file_str).await {
        Ok(offset) => offset,
        Err(err) => {
            tracing::warn!(error = %err, log_file = %active_file_str, "get_log_tail_offset failed");
            return;
        }
    };
    let since_offset = previous_offset.map(|offset| (active_file.clone(), offset));

    let (events, tailed_file, new_offset) = match log_tail::read_new_auth_failures(
        &config.log_dir,
        since_offset,
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::warn!(error = %err, log_dir = %config.log_dir.display(), "read_new_auth_failures failed");
            return;
        }
    };

    let now = Utc::now();
    for auth_failure in &events {
        let event = security::detect_auth_failure(auth_failure, now);
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, "record_event failed for a detected auth failure");
        }
    }

    let tailed_file_str = tailed_file.to_string_lossy().to_string();
    if let Err(err) = repository::set_log_tail_offset(repo, tailed_file_str, new_offset, now).await
    {
        tracing::warn!(error = %err, log_file = %active_file_str, "set_log_tail_offset failed");
    }
}

/// Unit U71 extracted this from `gucs`'s own handler body; unit U72
/// moved it here entirely and gave it its own real schedule (`spawn`
/// above) instead of `gucs` triggering it as a side effect of being
/// polled. `pub` so integration tests can invoke a single sweep
/// directly and deterministically, without waiting on a real timer —
/// the same reasoning `repository::run_retention_sweep` (unit U2)
/// being `pub` and callable outside its own scheduler already
/// establishes.
pub async fn run_security_config_sweep(
    repo: &repository::RepositoryHandle,
    adapter: &Arc<dyn DbmsAdapter>,
    gucs: &[CoreGucValue],
) {
    check_guc_changes(repo, gucs).await;
    check_role_superuser_grants(repo, adapter).await;
    check_role_membership_grants(repo, adapter).await;
    check_table_privilege_grants(repo, adapter).await;
    check_auth_failures(repo, adapter).await;
}
