//! `ActionKind` — what a real, mutating action type implements. **Zero
//! implementors ship in `core/src`** (see `core::policy`'s module doc's
//! scope note); the one implementor exercising this trait for real lives
//! in `core/tests/policy/common/mod.rs`. `risk_level` being a required
//! (non-defaulted) method is what makes SRS FR-POL-001's exhaustiveness
//! guarantee ("an action type with no assigned risk cannot exist in a
//! shipped build") a compile error rather than a runtime gap.

use serde_json::Value;

use super::error::ActionError;
use crate::policy::RiskLevel;

pub trait ActionKind: Send + Sync {
    fn action_type(&self) -> &'static str;

    fn risk_level(&self) -> RiskLevel;

    /// Unit U11 (SRS FR-SEC-003): whether this action type IS itself a
    /// security response (`core::security::response`'s closed set) —
    /// `executor::execute`'s security-flag stage never blocks these
    /// against their own flagged resource, only "performance-motivated"
    /// actions. Default `false`: only a real security response action
    /// overrides it.
    fn is_security_response(&self) -> bool {
        false
    }

    /// TRS §26 stage 4. Default `Ok(())` — most action types need no extra
    /// runtime capability probe beyond what construction already implies;
    /// override when one is needed.
    fn check_capability(&self) -> Result<(), ActionError> {
        Ok(())
    }

    /// TRS §26 stage 6 — the state `rollback` will need to restore.
    fn capture_previous_state(&self) -> Result<Value, ActionError>;

    /// TRS §26 stage 7.
    fn apply(&self) -> Result<(), ActionError>;

    /// TRS §26 stage 8.
    fn verify(&self) -> Result<(), ActionError>;

    /// Not part of the fixed nine-stage pipeline itself — invoked by
    /// `core::actions::rollback` on a separately re-verified target
    /// (TRS §27 / SRS FR-BENCH-003).
    fn rollback(&self, previous_state: &Value) -> Result<(), ActionError>;
}
