use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeviceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub devices: Vec<DeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeviceInfo {
    pub identifier: String,
    pub kind: DeviceKind,
    pub name: String,
    pub removable: bool,
    /// Whether this device was already known to the service before this
    /// observation. Ephemeral — an in-process set that resets on service
    /// restart until unit U2 adds real persistence; not a design flaw.
    pub trusted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeviceKind {
    BlockStorage,
    RemovableStorage,
}
