//! The action executor (unit U7, TRS §26-27). `execute`'s only public
//! entry point accepts a `policy::PolicyApproved` — there is no other way
//! to reach `apply()` (TRS §24).
//!
//! **Unit U8** adds the first real, shipped `ActionKind`/`TargetVerifier`
//! implementations — `host` (process priority + CPU affinity, TRS §38's
//! "the two U8 actions") — plus [`ActionContext`], the shared dependency
//! bundle a real `construct` function needs and a bare `fn` pointer alone
//! can't capture.

pub mod context;
pub mod error;
pub mod executor;
pub mod kind;
pub mod rollback;
pub mod target_verifier;

// Reads `/proc` (via `core::telemetry::process`) and calls into
// `platform::linux` — not gated everywhere else in `core::actions` is
// gated, but real host actions have no meaning off Linux yet
// (`platform::{windows,macos}` stub the underlying capability until unit
// U12, see this module's own doc comment above).
#[cfg(target_os = "linux")]
pub mod host;

pub use context::ActionContext;
pub use error::ActionError;
pub use executor::{execute, Executed};
pub use kind::ActionKind;
pub use rollback::{rollback, rollback_by_row_id};
pub use target_verifier::TargetVerifier;
