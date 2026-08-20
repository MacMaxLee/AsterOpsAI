//! The first REAL (non-test-only) `TargetVerifier` (unit U8). Reads the
//! live process's real `/proc/[pid]/stat` field 22 via `platform::linux::
//! RealProcSource` — the same real-file-reading path `core::telemetry`'s
//! own sampler uses, and the same field-22 parse
//! (`telemetry::process::read_start_time_ticks`, extracted in this unit
//! specifically so this verifier and the sampler share one parser instead
//! of two).

use platform::linux::RealProcSource;

use crate::actions::{ActionError, TargetVerifier};
use crate::policy::TargetIdentity;
use crate::telemetry::process::read_start_time_ticks;

pub struct ProcessTargetVerifier;

impl TargetVerifier for ProcessTargetVerifier {
    fn verify(&self, expected: &TargetIdentity) -> Result<(), ActionError> {
        let TargetIdentity::Process {
            pid,
            start_time_ticks,
        } = *expected
        else {
            return Err(ActionError::TargetChanged);
        };
        match read_start_time_ticks(&RealProcSource, pid) {
            Some(live) if live == start_time_ticks => Ok(()),
            _ => Err(ActionError::TargetChanged),
        }
    }
}
