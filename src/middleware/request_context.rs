use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::error::{LocalErrorEnvelope, OpenAiErrorEnvelope};
use crate::middleware::auth::is_openai_route;

#[derive(Debug, Default)]
pub(crate) struct RequestContext {
    pub request_id: String,
    pub selected_voice: Option<String>,
    pub text_length: Option<usize>,
    pub error_code: Option<String>,
}

pub(crate) type SharedRequestContext = Arc<Mutex<RequestContext>>;

pub(crate) async fn track_request(mut request: Request, next: Next) -> Response {
    let request_context = Arc::new(Mutex::new(RequestContext {
        request_id: Uuid::new_v4().to_string(),
        selected_voice: None,
        text_length: None,
        error_code: None,
    }));
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let started_at = Instant::now();
    request
        .extensions_mut()
        .insert(Arc::clone(&request_context));

    let response = next.run(request).await;
    let mut response = enrich_error_response(path.as_str(), &request_context, response).await;

    if let Ok(request_id_header) = HeaderValue::from_str(&request_id(&request_context)) {
        response
            .headers_mut()
            .insert("X-Request-Id", request_id_header);
    }

    log_request_event(
        method.as_str(),
        path.as_str(),
        response.status().as_u16(),
        started_at.elapsed().as_secs_f64() * 1000.0,
        &request_context,
    );

    response
}

pub(crate) fn request_id(context: &SharedRequestContext) -> String {
    with_context(context, |request_context| {
        request_context.request_id.clone()
    })
}

pub(crate) fn set_selected_voice(context: &SharedRequestContext, voice: impl Into<String>) {
    with_context(context, |request_context| {
        request_context.selected_voice = Some(voice.into());
    });
}

pub(crate) fn set_text_length(context: &SharedRequestContext, text_length: usize) {
    with_context(context, |request_context| {
        request_context.text_length = Some(text_length);
    });
}

pub(crate) fn set_error_code(context: &SharedRequestContext, error_code: impl Into<String>) {
    with_context(context, |request_context| {
        request_context.error_code = Some(error_code.into());
    });
}

fn with_context<T>(
    context: &SharedRequestContext,
    callback: impl FnOnce(&mut RequestContext) -> T,
) -> T {
    let mut guard = match context.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            let recovered = poisoned.into_inner();
            warn!(
                request_id = %recovered.request_id,
                "request context mutex was poisoned; recovering"
            );
            recovered
        }
    };

    callback(&mut guard)
}

async fn enrich_error_response(
    path: &str,
    request_context: &SharedRequestContext,
    response: Response,
) -> Response {
    if response.status().is_success() {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let body_bytes = match to_bytes(body, usize::MAX).await {
        Ok(body_bytes) => body_bytes,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

    if is_openai_route(path) {
        if let Ok(envelope) = serde_json::from_slice::<OpenAiErrorEnvelope>(&body_bytes) {
            if let Some(error_code) = envelope.error.code.clone() {
                set_error_code(request_context, error_code);
            }

            parts.headers.remove(CONTENT_LENGTH);
            return Response::from_parts(parts, Body::from(body_bytes));
        }

        return Response::from_parts(parts, Body::from(body_bytes));
    }

    let mut envelope = match serde_json::from_slice::<LocalErrorEnvelope>(&body_bytes) {
        Ok(envelope) => envelope,
        Err(_) => return Response::from_parts(parts, Body::from(body_bytes)),
    };

    if envelope.error.request_id.is_none() {
        envelope.error.request_id = Some(request_id(request_context));
    }
    set_error_code(request_context, envelope.error.code.clone());

    let updated_body = match serde_json::to_vec(&envelope) {
        Ok(updated_body) => updated_body,
        Err(_) => return Response::from_parts(parts, Body::from(body_bytes)),
    };

    parts.headers.remove(CONTENT_LENGTH);
    if !parts.headers.contains_key(CONTENT_TYPE) {
        parts
            .headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    }

    Response::from_parts(parts, Body::from(updated_body))
}

fn log_request_event(
    method: &str,
    path: &str,
    status_code: u16,
    latency_ms: f64,
    request_context: &SharedRequestContext,
) {
    let (request_id, selected_voice, text_length, error_code) =
        with_context(request_context, |request_context| {
            (
                request_context.request_id.clone(),
                request_context.selected_voice.clone(),
                request_context.text_length,
                request_context.error_code.clone(),
            )
        });

    match status_code {
        500.. => error!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status_code,
            latency_ms,
            voice = ?selected_voice,
            text_length = ?text_length,
            error_code = ?error_code,
            "request_completed"
        ),
        400..=499 => warn!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status_code,
            latency_ms,
            voice = ?selected_voice,
            text_length = ?text_length,
            error_code = ?error_code,
            "request_completed"
        ),
        _ => info!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status_code,
            latency_ms,
            voice = ?selected_voice,
            text_length = ?text_length,
            error_code = ?error_code,
            "request_completed"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{AppError, AppErrorCode, OpenAiErrorBody};
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::Json;

    fn test_context() -> SharedRequestContext {
        Arc::new(Mutex::new(RequestContext {
            request_id: "req-123".to_string(),
            selected_voice: None,
            text_length: None,
            error_code: None,
        }))
    }

    #[tokio::test]
    async fn enrich_error_response_injects_request_id_into_local_errors() {
        let context = test_context();
        let response = AppError::new(
            StatusCode::BAD_REQUEST,
            AppErrorCode::Validation,
            "bad request",
        )
        .into_response();

        let response = enrich_error_response("/v1/text-to-speech", &context, response).await;

        assert_eq!(response.headers().get(CONTENT_LENGTH), None);
        assert_eq!(request_id(&context), "req-123");
        assert_eq!(
            with_context(&context, |request_context| request_context
                .error_code
                .clone()),
            Some("validation_error".to_string())
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let envelope: LocalErrorEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(envelope.error.code, "validation_error");
        assert_eq!(envelope.error.request_id.as_deref(), Some("req-123"));
    }

    #[tokio::test]
    async fn enrich_error_response_preserves_openai_error_shape() {
        let context = test_context();
        let original = OpenAiErrorEnvelope {
            error: OpenAiErrorBody {
                code: Some("authentication_failed".to_string()),
                message: "Missing or invalid API key".to_string(),
                error_type: "invalid_request_error".to_string(),
            },
        };
        let response = (StatusCode::UNAUTHORIZED, Json(&original)).into_response();

        let response = enrich_error_response("/v1/audio/speech", &context, response).await;

        assert_eq!(response.headers().get(CONTENT_LENGTH), None);
        assert_eq!(
            with_context(&context, |request_context| request_context
                .error_code
                .clone()),
            Some("authentication_failed".to_string())
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let envelope: OpenAiErrorEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            envelope.error.code.as_deref(),
            Some("authentication_failed")
        );
        assert_eq!(envelope.error.message, original.error.message);
        assert_eq!(envelope.error.error_type, original.error.error_type);
    }

    #[test]
    fn setter_helpers_update_request_context_fields() {
        let context = test_context();

        set_selected_voice(&context, "jasper");
        set_text_length(&context, 42);
        set_error_code(&context, "backend_unavailable");

        let snapshot = with_context(&context, |request_context| {
            (
                request_context.selected_voice.clone(),
                request_context.text_length,
                request_context.error_code.clone(),
            )
        });

        assert_eq!(snapshot.0.as_deref(), Some("jasper"));
        assert_eq!(snapshot.1, Some(42));
        assert_eq!(snapshot.2.as_deref(), Some("backend_unavailable"));
    }
}
