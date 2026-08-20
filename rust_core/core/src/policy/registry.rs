//! The exhaustive action-type mapping FR-POL-001 requires. Empty of real
//! entries in shipped code (SCOPE — see the U7 plan's own framing: real
//! action types are unit U8's job, registered only by this crate's tests
//! this unit).

use std::collections::HashMap;

use crate::actions::ActionKind;

use super::request::ActionRequest;
use super::risk::RiskLevel;

pub struct ActionTypeEntry {
    pub risk_level: RiskLevel,
    pub validate_parameters: fn(&serde_json::Value) -> Result<(), String>,
    pub construct: fn(&ActionRequest) -> Box<dyn ActionKind>,
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
