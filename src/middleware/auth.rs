use axum::extract::{Request, State};
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use std::collections::BTreeMap;

use crate::app_state::AppState;
use crate::error::{LocalErrorBody, LocalErrorEnvelope, OpenAiErrorBody, OpenAiErrorEnvelope};
use crate::middleware::request_context::{request_id, set_error_code, SharedRequestContext};

pub(crate) const PUBLIC_PATHS: [&str; 1] = ["/healthz"];
pub(crate) const PROTECTED_PREFIXES: [&str; 1] = ["/v1"];
pub(crate) const OPENAI_ROUTE_PATHS: [&str; 1] = ["/v1/audio/speech"];

pub(crate) fn requires_api_key(path: &str) -> bool {
    if PUBLIC_PATHS.contains(&path) {
        return false;
    }

    PROTECTED_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

pub(crate) fn is_openai_route(path: &str) -> bool {
    OPENAI_ROUTE_PATHS.contains(&path)
}

fn normalized_header_value(value: Option<&str>) -> Option<String> {
    let stripped = value?.trim();
    if stripped.is_empty() {
        return None;
    }

    Some(stripped.to_string())
}

fn extract_xi_api_key(headers: &HeaderMap) -> Option<String> {
    normalized_header_value(headers.get("xi-api-key")?.to_str().ok())
}

fn extract_bearer_api_key(headers: &HeaderMap) -> Option<String> {
    let authorization = normalized_header_value(headers.get(AUTHORIZATION)?.to_str().ok())?;
    let (scheme, token) = authorization
        .split_once(' ')
        .unwrap_or((&authorization, ""));
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }

    normalized_header_value(Some(token))
}

pub(crate) fn extract_api_key(headers: &HeaderMap) -> (Option<String>, bool) {
    let xi_api_key = extract_xi_api_key(headers);
    let bearer_api_key = extract_bearer_api_key(headers);

    if let (Some(xi_api_key), Some(bearer_api_key)) = (&xi_api_key, &bearer_api_key) {
        if xi_api_key != bearer_api_key {
            return (None, true);
        }
    }

    (xi_api_key.or(bearer_api_key), false)
}

pub(crate) fn is_request_authorized(
    settings: &crate::config::Settings,
    api_key: Option<&str>,
) -> bool {
    if !settings.auth_enabled {
        return true;
    }

    match (api_key, settings.local_api_key.as_deref()) {
        (Some(api_key), Some(expected_api_key)) => api_key == expected_api_key,
        _ => false,
    }
}

pub(crate) async fn authorize(
    State(state): State<AppState>,
    Extension(request_context): Extension<SharedRequestContext>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let (api_key, conflicting_headers) = extract_api_key(request.headers());

    if requires_api_key(&path)
        && (conflicting_headers || !is_request_authorized(&state.settings, api_key.as_deref()))
    {
        set_error_code(&request_context, "authentication_failed");
        return authentication_error_response(
            &path,
            request_id(&request_context),
            conflicting_headers,
        );
    }

    next.run(request).await
}

fn authentication_error_response(
    path: &str,
    request_id: String,
    conflicting_headers: bool,
) -> Response {
    let message = if conflicting_headers {
        "Conflicting API key headers"
    } else if is_openai_route(path) {
        "Missing or invalid API key"
    } else {
        "Missing or invalid xi-api-key"
    };

    if is_openai_route(path) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(OpenAiErrorEnvelope {
                error: OpenAiErrorBody {
                    code: Some("authentication_failed".to_string()),
                    message: message.to_string(),
                    error_type: "invalid_request_error".to_string(),
                },
            }),
        )
            .into_response();
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(LocalErrorEnvelope {
            error: LocalErrorBody {
                code: "authentication_failed".to_string(),
                message: message.to_string(),
                details: BTreeMap::new(),
                request_id: Some(request_id),
            },
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use axum::http::HeaderValue;

    #[test]
    fn extract_api_key_prefers_matching_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("xi-api-key", HeaderValue::from_static("secret"));
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));

        let (api_key, conflicting) = extract_api_key(&headers);

        assert_eq!(api_key.as_deref(), Some("secret"));
        assert!(!conflicting);
    }

    #[test]
    fn extract_api_key_detects_conflicting_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("xi-api-key", HeaderValue::from_static("secret-a"));
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret-b"));

        let (api_key, conflicting) = extract_api_key(&headers);

        assert!(api_key.is_none());
        assert!(conflicting);
    }

    #[test]
    fn extract_api_key_accepts_case_insensitive_bearer_and_trims_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("  bEaReR   secret-token  "),
        );

        let (api_key, conflicting) = extract_api_key(&headers);

        assert_eq!(api_key.as_deref(), Some("secret-token"));
        assert!(!conflicting);
    }

    #[test]
    fn extract_api_key_ignores_non_bearer_authorization_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic secret"));

        let (api_key, conflicting) = extract_api_key(&headers);

        assert!(api_key.is_none());
        assert!(!conflicting);
    }

    #[test]
    fn extract_api_key_ignores_blank_header_values() {
        let mut headers = HeaderMap::new();
        headers.insert("xi-api-key", HeaderValue::from_static("   "));
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer    "));

        let (api_key, conflicting) = extract_api_key(&headers);

        assert!(api_key.is_none());
        assert!(!conflicting);
    }

    #[test]
    fn requires_api_key_keeps_health_public() {
        assert!(!requires_api_key("/healthz"));
        assert!(requires_api_key("/v1/voices"));
    }

    #[test]
    fn route_classification_uses_exact_public_and_openai_paths() {
        assert!(!requires_api_key("/healthz/extra"));
        assert!(requires_api_key("/v1"));
        assert!(requires_api_key("/v1beta"));
        assert!(!requires_api_key("/voices"));

        assert!(is_openai_route("/v1/audio/speech"));
        assert!(!is_openai_route("/v1/audio/speech/extra"));
    }

    #[test]
    fn is_request_authorized_requires_matching_key_when_enabled() {
        let settings = Settings {
            auth_enabled: true,
            local_api_key: Some("secret".to_string()),
            ..Settings::default()
        };

        assert!(is_request_authorized(&settings, Some("secret")));
        assert!(!is_request_authorized(&settings, Some("other")));
        assert!(!is_request_authorized(&settings, None));
    }

    #[test]
    fn is_request_authorized_allows_requests_when_auth_is_disabled() {
        let settings = Settings {
            auth_enabled: false,
            local_api_key: None,
            ..Settings::default()
        };

        assert!(is_request_authorized(&settings, None));
        assert!(is_request_authorized(&settings, Some("anything")));
    }
}
