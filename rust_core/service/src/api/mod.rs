pub mod v1;

use axum::Router;
use tower_http::trace::TraceLayer;

use crate::middleware::assign_request_id;
use crate::state::AppState;

/// Builds the full router, versioned under `/api/v1` (requirement 4). Adding
/// `/api/v2` later is a sibling `.nest()` call, not a restructuring.
pub fn router(state: AppState) -> Router {
    Router::new()
        .nest("/api/v1", v1::router())
        .with_state(state)
        .layer(axum::middleware::from_fn(assign_request_id))
        .layer(TraceLayer::new_for_http())
}
