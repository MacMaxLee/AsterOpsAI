//! The exhaustive action-type mapping FR-POL-001 requires. Populated for
//! real starting unit U8 (`core::actions::host::{set_process_priority_entry,
//! set_process_cpu_affinity_entry}`) — registration itself still happens
//! at the call site (this crate's tests, or a future `service` wiring
//! unit), not inside this module.
//!
//! `construct` takes `&ActionContext` (unit U8) — a bare `fn` pointer
//! can't capture a shared dependency like a live `PlatformAdapter`, so the
//! caller (`policy::approval::authorize`) supplies one explicitly instead.
//! It's fallible (`Result`, not the plain `Box<dyn ActionKind>` U7
//! originally shipped) because real construction can genuinely fail in a
//! way `validate_parameters` can't catch — e.g. an action type registered
//! for `Process` targets receiving a `DbSession` one; `validate_parameters`
//! only ever sees the parameter JSON, never the target.

use std::collections::HashMap;

use crate::actions::{ActionContext, ActionKind};

use super::request::ActionRequest;
use super::risk::RiskLevel;

/// Named purely to keep `ActionTypeEntry.construct`'s declaration readable
/// (clippy's `type_complexity` lint) — not a meaningfully reusable
/// abstraction on its own.
pub type ConstructFn = fn(&ActionRequest, &ActionContext) -> Result<Box<dyn ActionKind>, String>;

pub struct ActionTypeEntry {
    pub risk_level: RiskLevel,
    pub validate_parameters: fn(&serde_json::Value) -> Result<(), String>,
    pub construct: ConstructFn,
}

#[derive(Default)]
pub struct ActionTypeRegistry {
    entries: HashMap<String, ActionTypeEntry>,
}

impl ActionTypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, action_type: impl Into<String>, entry: ActionTypeEntry) {
        self.entries.insert(action_type.into(), entry);
    }

    pub fn lookup(&self, action_type: &str) -> Option<&ActionTypeEntry> {
        self.entries.get(action_type)
    }
}
