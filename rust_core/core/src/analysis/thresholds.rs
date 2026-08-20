//! Deterministic tier thresholds new to unit U5 — judgment calls not pinned
//! by SRS/TRS, documented in docs/adr/0010-performance-analysis-thresholds-
//! and-scope.md, exactly like CPU/memory pressure's own precedent in ADR
//! 0006. Boundary semantics match ADR 0006's CPU table: `<` for
//! NORMAL/ELEVATED/HIGH, `>=` for CRITICAL — pinned by the boundary tests
//! below, not just prose.

/// A generic 4-tier severity, shared by every threshold in this module
/// (host domains that don't already have a `contracts` pressure enum, and
/// every DB health category) so `host.rs`/`db.rs` combine signals uniformly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Normal,
    Elevated,
    High,
    Critical,
}

impl Tier {
    pub fn is_crossed(self) -> bool {
        self >= Tier::High
    }
}

fn tier_for(value: f64, elevated: f64, high: f64, critical: f64) -> Tier {
    if value >= critical {
        Tier::Critical
    } else if value >= high {
        Tier::High
    } else if value >= elevated {
        Tier::Elevated
    } else {
        Tier::Normal
    }
}

// Host: STORAGE_IO — per-volume io_latency_ms, worst volume drives the
// sample's signal.
pub const STORAGE_IO_LATENCY_MS_ELEVATED: f64 = 10.0;
pub const STORAGE_IO_LATENCY_MS_HIGH: f64 = 30.0;
pub const STORAGE_IO_LATENCY_MS_CRITICAL: f64 = 80.0;

pub fn classify_storage_io_tier(latency_ms: f64) -> Tier {
    tier_for(
        latency_ms,
        STORAGE_IO_LATENCY_MS_ELEVATED,
        STORAGE_IO_LATENCY_MS_HIGH,
        STORAGE_IO_LATENCY_MS_CRITICAL,
    )
}

// Host: NETWORK — (rx+tx errors+drops per sec) / (rx+tx packets per sec),
// a link-speed-independent signal (no interface bandwidth is known).
pub const NETWORK_ERROR_RATIO_ELEVATED: f64 = 0.0001; // 0.01%
pub const NETWORK_ERROR_RATIO_HIGH: f64 = 0.001; // 0.1%
pub const NETWORK_ERROR_RATIO_CRITICAL: f64 = 0.01; // 1%

pub fn classify_network_error_tier(error_ratio: f64) -> Tier {
    tier_for(
        error_ratio,
        NETWORK_ERROR_RATIO_ELEVATED,
        NETWORK_ERROR_RATIO_HIGH,
        NETWORK_ERROR_RATIO_CRITICAL,
    )
}

// Host window sustained-signal rule (FR-SYS-003's back-off spirit: react to
// sustained pressure, not single-sample noise).
pub const MIN_SAMPLES_FOR_CLASSIFICATION: usize = 3;
pub const SUSTAINED_FRACTION: f64 = 0.6;

// Host: BACKGROUND — a crossed CPU/MEMORY domain is reattributed to
// BACKGROUND when BackgroundService-categorized processes account for at
// least this fraction of the domain's live usage.
pub const BACKGROUND_MAJORITY_FRACTION: f64 = 0.5;

// DB: connection saturation — sessions / max_connections.
pub const DB_CONN_SATURATION_ELEVATED: f64 = 0.70;
pub const DB_CONN_SATURATION_HIGH: f64 = 0.85;
pub const DB_CONN_SATURATION_CRITICAL: f64 = 0.95;

pub fn classify_connection_saturation_tier(ratio: f64) -> Tier {
    tier_for(
        ratio,
        DB_CONN_SATURATION_ELEVATED,
        DB_CONN_SATURATION_HIGH,
        DB_CONN_SATURATION_CRITICAL,
    )
}

// DB: latency — mean_exec_time_ms of the slowest tracked statement. Also
// the threshold "slow query" count uses to decide what's slow.
pub const DB_LATENCY_MS_ELEVATED: f64 = 100.0;
pub const DB_LATENCY_MS_HIGH: f64 = 500.0;
pub const DB_LATENCY_MS_CRITICAL: f64 = 2000.0;

pub fn classify_latency_tier(mean_exec_time_ms: f64) -> Tier {
    tier_for(
        mean_exec_time_ms,
        DB_LATENCY_MS_ELEVATED,
        DB_LATENCY_MS_HIGH,
        DB_LATENCY_MS_CRITICAL,
    )
}

// DB: slow query count — statements at/above DB_LATENCY_MS_HIGH.
pub const DB_SLOW_QUERY_COUNT_ELEVATED: f64 = 1.0;
pub const DB_SLOW_QUERY_COUNT_HIGH: f64 = 3.0;
pub const DB_SLOW_QUERY_COUNT_CRITICAL: f64 = 10.0;

pub fn classify_slow_query_count_tier(count: f64) -> Tier {
    tier_for(
        count,
        DB_SLOW_QUERY_COUNT_ELEVATED,
        DB_SLOW_QUERY_COUNT_HIGH,
        DB_SLOW_QUERY_COUNT_CRITICAL,
    )
}

// DB: lock waits — waiting sessions + blocking edges.
pub const DB_LOCK_WAIT_COUNT_ELEVATED: f64 = 1.0;
pub const DB_LOCK_WAIT_COUNT_HIGH: f64 = 5.0;
pub const DB_LOCK_WAIT_COUNT_CRITICAL: f64 = 15.0;

pub fn classify_lock_wait_count_tier(count: f64) -> Tier {
    tier_for(
        count,
        DB_LOCK_WAIT_COUNT_ELEVATED,
        DB_LOCK_WAIT_COUNT_HIGH,
        DB_LOCK_WAIT_COUNT_CRITICAL,
    )
}

// DB: deadlocks — absolute count since stats_reset (no rate available, see
// module doc on core::dbms::DeadlockInfo).
pub const DB_DEADLOCK_COUNT_ELEVATED: f64 = 1.0;
pub const DB_DEADLOCK_COUNT_HIGH: f64 = 3.0;
pub const DB_DEADLOCK_COUNT_CRITICAL: f64 = 10.0;

pub fn classify_deadlock_count_tier(count: f64) -> Tier {
    tier_for(
        count,
        DB_DEADLOCK_COUNT_ELEVATED,
        DB_DEADLOCK_COUNT_HIGH,
        DB_DEADLOCK_COUNT_CRITICAL,
    )
}

// DB: rollback ratio — xact_rollback / (xact_commit + xact_rollback).
pub const DB_ROLLBACK_RATIO_ELEVATED: f64 = 0.02;
pub const DB_ROLLBACK_RATIO_HIGH: f64 = 0.05;
pub const DB_ROLLBACK_RATIO_CRITICAL: f64 = 0.15;

pub fn classify_rollback_ratio_tier(ratio: f64) -> Tier {
    tier_for(
        ratio,
        DB_ROLLBACK_RATIO_ELEVATED,
        DB_ROLLBACK_RATIO_HIGH,
        DB_ROLLBACK_RATIO_CRITICAL,
    )
}

// DB: temp file usage — total temp_bytes since stats_reset (absolute, same
// reason as deadlocks).
pub const DB_TEMP_BYTES_ELEVATED: f64 = 100.0 * 1024.0 * 1024.0; // 100 MiB
pub const DB_TEMP_BYTES_HIGH: f64 = 1024.0 * 1024.0 * 1024.0; // 1 GiB
pub const DB_TEMP_BYTES_CRITICAL: f64 = 10.0 * 1024.0 * 1024.0 * 1024.0; // 10 GiB

pub fn classify_temp_bytes_tier(bytes: f64) -> Tier {
    tier_for(
        bytes,
        DB_TEMP_BYTES_ELEVATED,
        DB_TEMP_BYTES_HIGH,
        DB_TEMP_BYTES_CRITICAL,
    )
}

// DB: long transactions — count of transactions already past the query's
// own duration floor (see core::dbms::adapters::postgresql::queries::
// long_transactions).
pub const DB_LONG_TXN_COUNT_ELEVATED: f64 = 1.0;
pub const DB_LONG_TXN_COUNT_HIGH: f64 = 3.0;
pub const DB_LONG_TXN_COUNT_CRITICAL: f64 = 10.0;

pub fn classify_long_transaction_count_tier(count: f64) -> Tier {
    tier_for(
        count,
        DB_LONG_TXN_COUNT_ELEVATED,
        DB_LONG_TXN_COUNT_HIGH,
        DB_LONG_TXN_COUNT_CRITICAL,
    )
}

// DB: replication lag — max replay_lag_seconds across standbys.
pub const DB_REPLICATION_LAG_SECONDS_ELEVATED: f64 = 5.0;
pub const DB_REPLICATION_LAG_SECONDS_HIGH: f64 = 30.0;
pub const DB_REPLICATION_LAG_SECONDS_CRITICAL: f64 = 120.0;

pub fn classify_replication_lag_tier(seconds: f64) -> Tier {
    tier_for(
        seconds,
        DB_REPLICATION_LAG_SECONDS_ELEVATED,
        DB_REPLICATION_LAG_SECONDS_HIGH,
        DB_REPLICATION_LAG_SECONDS_CRITICAL,
    )
}

// DB: bloat proxy — max(n_dead_tup / (n_live_tup + n_dead_tup)) among
// tables with at least this many total rows (guards noise on tiny tables).
pub const DB_BLOAT_MIN_ROWS: i64 = 1000;
pub const DB_BLOAT_RATIO_ELEVATED: f64 = 0.10;
pub const DB_BLOAT_RATIO_HIGH: f64 = 0.20;
pub const DB_BLOAT_RATIO_CRITICAL: f64 = 0.40;

pub fn classify_bloat_ratio_tier(ratio: f64) -> Tier {
    tier_for(
        ratio,
        DB_BLOAT_RATIO_ELEVATED,
        DB_BLOAT_RATIO_HIGH,
        DB_BLOAT_RATIO_CRITICAL,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_io_boundaries() {
        assert_eq!(classify_storage_io_tier(0.0), Tier::Normal);
        assert_eq!(classify_storage_io_tier(9.999), Tier::Normal);
        assert_eq!(classify_storage_io_tier(10.0), Tier::Elevated);
        assert_eq!(classify_storage_io_tier(29.999), Tier::Elevated);
        assert_eq!(classify_storage_io_tier(30.0), Tier::High);
        assert_eq!(classify_storage_io_tier(79.999), Tier::High);
        assert_eq!(classify_storage_io_tier(80.0), Tier::Critical);
    }

    #[test]
    fn network_error_ratio_boundaries() {
        assert_eq!(classify_network_error_tier(0.0), Tier::Normal);
        assert_eq!(classify_network_error_tier(0.0001), Tier::Elevated);
        assert_eq!(classify_network_error_tier(0.001), Tier::High);
        assert_eq!(classify_network_error_tier(0.01), Tier::Critical);
    }

    #[test]
    fn tier_is_crossed_only_at_high_or_above() {
        assert!(!Tier::Normal.is_crossed());
        assert!(!Tier::Elevated.is_crossed());
        assert!(Tier::High.is_crossed());
        assert!(Tier::Critical.is_crossed());
    }

    #[test]
    fn db_category_boundaries_all_have_working_tier_edges() {
        assert_eq!(classify_connection_saturation_tier(0.94999), Tier::High);
        assert_eq!(classify_connection_saturation_tier(0.95), Tier::Critical);
        assert_eq!(classify_latency_tier(499.999), Tier::Elevated);
        assert_eq!(classify_latency_tier(500.0), Tier::High);
        assert_eq!(classify_slow_query_count_tier(3.0), Tier::High);
        assert_eq!(classify_lock_wait_count_tier(5.0), Tier::High);
        assert_eq!(classify_deadlock_count_tier(3.0), Tier::High);
        assert_eq!(classify_rollback_ratio_tier(0.05), Tier::High);
        assert_eq!(classify_temp_bytes_tier(DB_TEMP_BYTES_HIGH), Tier::High);
        assert_eq!(classify_long_transaction_count_tier(3.0), Tier::High);
        assert_eq!(classify_replication_lag_tier(30.0), Tier::High);
        assert_eq!(classify_bloat_ratio_tier(0.20), Tier::High);
    }
}
