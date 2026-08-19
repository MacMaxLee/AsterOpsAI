use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Closed, wire-facing error type. Every API error response carries exactly
/// one of these variants — never a bare string — so clients can branch on
/// `code` exhaustively instead of pattern-matching on message text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[serde(tag = "code", content = "message", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("not found")]
    NotFound,

    #[error("unsupported on this platform: {0}")]
    Unsupported(String),

    #[error("permission required: {0}")]
    PermissionRequired(String),

    #[error("temporarily unavailable: {0}")]
    Unavailable(String),

    #[error("internal error")]
    Internal,
}
