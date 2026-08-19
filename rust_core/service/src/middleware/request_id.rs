use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

/// A per-request identifier attached to every request's extensions by
/// [`assign_request_id`], echoed both in the response `Envelope` body and the
/// `x-request-id` response header.
#[derive(Debug, Clone, Copy)]
pub struct RequestId(pub Uuid);

pub async fn assign_request_id(mut req: Request, next: Next) -> Response {
    let request_id = RequestId(Uuid::new_v4());
    req.extensions_mut().insert(request_id);

    let mut response = next.run(req).await;
    if let Ok(value) = HeaderValue::from_str(&request_id.0.to_string()) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}
