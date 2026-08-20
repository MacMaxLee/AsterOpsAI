//! Stage 1 of TRS §26's pipeline: schema validation.

use super::error::PolicyError;
use super::registry::ActionTypeRegistry;
use super::request::ActionRequest;
use super::typestate::Validated;

pub fn validate(
    request: ActionRequest,
    registry: &ActionTypeRegistry,
) -> Result<Validated<ActionRequest>, PolicyError> {
    let entry = registry
        .lookup(&request.action_type)
        .ok_or_else(|| PolicyError::UnknownActionType(request.action_type.clone()))?;
    (entry.validate_parameters)(&request.parameters).map_err(|reason| {
        PolicyError::InvalidParameters {
            action_type: request.action_type.clone(),
            reason,
        }
    })?;
    Ok(Validated::new(request))
}
