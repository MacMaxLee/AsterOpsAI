//! Extension/version-gated capability detection (requirement 6): a missing
//! `pg_stat_statements` degrades `query_stats()` to `Unavailable` with a
//! reason — never a silent empty `Vec`, never a `CREATE EXTENSION` attempt.
//!
//! `contracts::Capability` (reused everywhere else in this codebase for
//! exactly this state family) can't be reused verbatim here because it
//! carries no payload — `Supported` has no associated value, so there's
//! nowhere to put the `Vec<QueryStat>` when the capability *is* available.
//! `Gated<T>` is the same four-state shape with a payload added to the
//! `Supported` case; SCOPE keeps this crate-internal (`core::dbms`) rather
//! than adding a new generic to `contracts`, since there's no API layer
//! consuming it yet (see mod.rs's scope note).

#[derive(Debug, Clone, PartialEq)]
pub enum Gated<T> {
    Supported(T),
    Limited { reason: String },
    Unavailable { reason: String },
    PermissionRequired { reason: String },
}

impl<T> Gated<T> {
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported(_))
    }
}
