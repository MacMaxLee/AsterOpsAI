use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

/// Every API response is wrapped in this envelope: `{success, timestamp,
/// request_id, data, error}`. Exactly one of `data`/`error` is populated,
/// matching `success`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Envelope<T> {
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    pub request_id: Uuid,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

impl<T> Envelope<T> {
    pub fn ok(request_id: Uuid, data: T) -> Self {
        Self {
            success: true,
            timestamp: Utc::now(),
            request_id,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(request_id: Uuid, error: ApiError) -> Self {
        Self {
            success: false,
            timestamp: Utc::now(),
            request_id,
            data: None,
            error: Some(error),
        }
    }
}
