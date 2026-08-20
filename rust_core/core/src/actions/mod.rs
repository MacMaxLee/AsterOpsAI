//! The action executor (unit U7, TRS §26-27). `execute`'s only public
//! entry point accepts a `policy::PolicyApproved` — there is no other way
//! to reach `apply()` (TRS §24).
//!
//! **Scope note**: `ActionKind`/`TargetVerifier` are traits only — zero
//! production implementations ship here (see `core::policy`'s module doc's
//! scope note: real action types and real target verifiers are unit U8's
//! job, gated through what this unit builds).

pub mod error;
pub mod executor;
pub mod kind;
pub mod rollback;
pub mod target_verifier;

pub use error::ActionError;
pub use executor::{execute, Executed};
pub use kind::ActionKind;
pub use rollback::rollback;
pub use target_verifier::TargetVerifier;
