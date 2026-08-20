//! Performance analysis: deterministic host bottleneck classification and
//! database health classification (unit U5, SRS §11/§26-§34, TRS §21).
//!
//! **Hard boundary (SRS FR-PERF-005)**: nothing in this module may call,
//! import, or otherwise be influenced by an AI model. Every function here
//! is pure arithmetic over caller-supplied evidence — no I/O, no network
//! access, no dependency on any AI-provider type.
//!
//! **Scope note**: this is the classification/scoring layer only — not
//! wired into a live poll loop, `service`'s HTTP surface, or the console
//! (see the U5 plan's SCOPE note, same precedent U4 set for `core::dbms`).
//! Cross-layer correlation (SRS §31/TRS §20) is a distinct, later unit that
//! consumes this module's output types as input, not part of this module.

pub mod db;
pub mod evidence;
pub mod host;
pub mod score;
pub mod thresholds;

pub use db::{
    classify_db, unavailable_verdict, DbEvidenceBundle, DbHealthCategory, DbHealthCheck,
    DbHealthStatus, DbHealthVerdict,
};
pub use evidence::Evidence;
pub use host::{classify_host, DomainSignal, HostBottleneck, HostDomain, HostVerdict};
pub use score::{DB_SCORE_VERSION, HOST_SCORE_VERSION};
