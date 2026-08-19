//! Version support matrix (requirement 8): declares the oldest PostgreSQL
//! server this adapter supports, and how an individual capability degrades
//! on an older server rather than the whole connection erroring out.

/// `server_version_num`'s encoding: PG 13.0 is `130000`. Matches the DoD's
/// own oldest tested version (13/15/17), not picked independently of it.
pub const MIN_SUPPORTED_PG_VERSION: i32 = 130000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PgVersion(pub i32);

impl PgVersion {
    pub fn is_supported(self) -> bool {
        self.0 >= MIN_SUPPORTED_PG_VERSION
    }

    /// `pg_blocking_pids()` (used by `lock_graph`) exists since PG 9.6 —
    /// safe unconditionally across the whole 13/15/17 support window, but
    /// named explicitly so a future lowering of `MIN_SUPPORTED_PG_VERSION`
    /// below 9.6 (never expected) would have an obvious place to add a
    /// real version check.
    pub fn supports_blocking_pids(self) -> bool {
        self.0 >= 90600
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg13_is_the_supported_floor() {
        assert!(PgVersion(MIN_SUPPORTED_PG_VERSION).is_supported());
        assert!(!PgVersion(MIN_SUPPORTED_PG_VERSION - 1).is_supported());
    }
}
