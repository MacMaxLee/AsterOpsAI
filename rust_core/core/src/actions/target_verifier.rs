//! TRS §27: "Identity is re-verified immediately before apply *and*
//! immediately before rollback; a mismatch aborts with `TARGET_CHANGED`."
//! `TargetVerifier` is the abstraction the executor calls at both points.
//! **Zero production implementations ship in `core/src`** — a real
//! implementation (reading a live process's `/proc/[pid]/stat` field 22,
//! or a live DB backend's start time) is action-surface work, unit U8's
//! job per the same scope boundary `core::policy`'s module doc states;
//! `core/tests/policy/common/mod.rs` has the one real, test-only
//! implementation this unit's tests use to prove the mechanism works.

use super::error::ActionError;
use crate::policy::TargetIdentity;

pub trait TargetVerifier: Send + Sync {
    /// `Err(ActionError::TargetChanged)` when the live target no longer
    /// matches `expected` — any other error variant is a bug in the
    /// verifier, not a legitimate outcome of this check.
    fn verify(&self, expected: &TargetIdentity) -> Result<(), ActionError>;
}
