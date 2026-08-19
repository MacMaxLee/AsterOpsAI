/// Internal, per-domain error type for platform operations. Distinct from
/// `contracts::ApiError` (the closed, wire-facing enum) — this crosses into
/// the wire type only at the service boundary, via [`CapabilityError::to_capability`].
#[derive(Debug, thiserror::Error)]
pub enum CapabilityError {
    #[error("unsupported on this platform: {0}")]
    Unsupported(String),

    #[error("permission required: {0}")]
    PermissionRequired(String),

    #[error("unavailable: {0}")]
    Unavailable(String),

    #[error("platform I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl CapabilityError {
    pub fn to_capability(&self) -> contracts::Capability {
        use contracts::Capability;
        match self {
            Self::Unsupported(reason) => Capability::Unavailable {
                reason: reason.clone(),
            },
            Self::PermissionRequired(reason) => Capability::PermissionRequired {
                reason: reason.clone(),
            },
            Self::Unavailable(reason) => Capability::Unavailable {
                reason: reason.clone(),
            },
            Self::Io(err) => Capability::Unavailable {
                reason: err.to_string(),
            },
        }
    }
}
