//! Security Detector Framework (unit U11, SRS §20 FR-SEC-001..005, TRS
//! §37).
//!
//! **Hard boundary (FR-SEC-001)**: `detector` is pure, deterministic
//! logic over caller-supplied evidence — no I/O, no AI dependency of any
//! kind, same boundary `core::analysis`'s own FR-PERF-005 note
//! establishes for performance classification.
//!
//! **Hard boundary (FR-SEC-002)**: related events are correlated into a
//! single incident, never reported as unrelated noise — the real
//! find-or-open logic lives in `repository::security::insert_event`, run
//! atomically through the single writer thread (ADR 0007).
//!
//! **Hard boundary (FR-SEC-003)**: a security-relevant flag on a
//! resource overrides any performance-motivated action targeting it —
//! enforced as `actions::executor::execute`'s own dedicated stage (the
//! same "freshest possible check, at execute-time" precedent ADR 0012
//! already set for the protected-resource stage), never ad hoc at a call
//! site.
//!
//! **Hard boundary (FR-SEC-004)**: [`response::SecurityResponseAction`]
//! is the only sanctioned way a security-response action type gets
//! registered — a closed, exhaustively-matched set, real structure, not
//! documentation-only.
//!
//! **Scope note**: no `service`/console wiring happens in this unit —
//! detectors aren't hooked into a live poll loop, same incremental-
//! layering precedent every unit since U4 has kept.

pub mod detector;
pub mod error;
pub mod incident;
pub mod severity;
pub mod suppression;

#[cfg(target_os = "linux")]
pub mod response;

pub use detector::{
    detect_superuser_override, detect_untrusted_device, DetectedEvent, DETECTOR_SUPERUSER_OVERRIDE,
    DETECTOR_UNTRUSTED_DEVICE,
};
pub use error::SecurityError;
pub use incident::{record_event, IncidentOutcome};
pub use severity::Severity;
pub use suppression::suppress;
