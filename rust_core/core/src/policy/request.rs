use serde_json::Value;

use super::resource::ResourceDescriptor;
use super::target::TargetIdentity;

/// The raw, untyped starting point of TRS §24's typestate chain —
/// `ActionRequest -> Validated<ActionRequest> -> PolicyApproved -> Executed`.
#[derive(Debug, Clone)]
pub struct ActionRequest {
    pub action_type: String,
    pub target: TargetIdentity,
    pub resource: ResourceDescriptor,
    pub parameters: Value,
    pub requested_by: String,
}
