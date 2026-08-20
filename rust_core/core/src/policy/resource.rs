//! Protected-resource registry (SRS FR-POL-006): "a registry of protected
//! processes, protected services, trusted devices, and critical
//! infrastructure is consulted by the policy engine — never by ad hoc
//! checks at the call site — before any mutating action is approved or
//! executed." Genuinely new: `DeviceInfo.trusted` (U1) is ephemeral
//! first-seen tracking with no persistence and no operator input, not this.
//! Checked once, at execution time (the executor pipeline's own dedicated
//! stage, TRS §26) rather than duplicated at proposal time too — see
//! docs/adr/0012 for why a single, freshest-possible check is more correct
//! than an early one that could go stale between proposal and execution.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceKind {
    Process,
    Service,
    Device,
    Infrastructure,
}

/// What an `ActionRequest` names as the resource it targets — separate
/// from `TargetIdentity` (a `pid`/`backend_pid` is ephemeral and reused;
/// this is the semantic name — a process command, a service name, a
/// device identifier — a protection entry is actually keyed on).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceDescriptor {
    pub kind: ResourceKind,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProtectedResourceRegistry {
    protected: Vec<ResourceDescriptor>,
}

impl ProtectedResourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn protect(&mut self, descriptor: ResourceDescriptor) {
        self.protected.push(descriptor);
    }

    /// `Err` if `descriptor` matches a protected entry of the same kind —
    /// exact name match, no pattern/glob matching this unit (a documented,
    /// deliberately narrow scope; a future unit can widen it without
    /// changing this signature).
    pub fn check(&self, descriptor: &ResourceDescriptor) -> Result<(), String> {
        if self.protected.contains(descriptor) {
            return Err(format!(
                "{:?} {:?} is a protected resource",
                descriptor.kind, descriptor.name
            ));
        }
        Ok(())
    }
}
