use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use kittentts_server_rs::{build_router, AppState, EngineMetadata, Settings};
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;

#[tokio::test]
async fn health_route_returns_server_metadata() {
    let mut engine_metadata = EngineMetadata::new("kitten_tts_rs", "0.1.0", false);
    engine_metadata.onnx_runtime_source = Some("local_discovery".to_string());
    engine_metadata.onnx_runtime_path = Some(PathBuf::from(
        "/home/test/.local/share/onnxruntime/1.24.2/libonnxruntime.so.1.24.2",
    ));
    let state = AppState::new(Settings::default(), engine_metadata);
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_text.contains("\"status\":\"ok\""));
    assert!(body_text.contains("\"onnx_runtime_source\":\"local_discovery\""));
    assert!(body_text.contains("\"onnx_runtime_path\":\"/home/test/.local/share/onnxruntime/1.24.2/libonnxruntime.so.1.24.2\""));
    assert!(body_text.contains("\"default_voice_id\":\"jasper\""));
}

#[tokio::test]
async fn health_route_is_public_and_sets_request_id_header() {
    let settings = Settings {
        auth_enabled: true,
        local_api_key: Some("secret".to_string()),
        ..Settings::default()
    };
    let state = AppState::new(
        settings,
        EngineMetadata::new("kitten_tts_rs", "0.1.0", false),
    );
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("X-Request-Id").is_some());
}

#[tokio::test]
async fn protected_route_rejects_missing_api_key_with_local_error() {
    let settings = Settings {
        auth_enabled: true,
        local_api_key: Some("secret".to_string()),
        ..Settings::default()
    };
    let state = AppState::new(
        settings,
        EngineMetadata::new("kitten_tts_rs", "0.1.0", false),
    );
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/voices")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get("X-Request-Id").is_some());

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body_json["error"]["code"], "authentication_failed");
    assert_eq!(
        body_json["error"]["message"],
        "Missing or invalid xi-api-key"
    );
    assert!(body_json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn protected_route_allows_requests_when_auth_is_disabled() {
    let settings = Settings {
        auth_enabled: false,
        local_api_key: Some("secret".to_string()),
        ..Settings::default()
    };
    let state = AppState::new(
        settings,
        EngineMetadata::new("kitten_tts_rs", "0.1.0", false),
    );
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/voices")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn protected_route_accepts_xi_api_key() {
    let settings = Settings {
        auth_enabled: true,
        local_api_key: Some("secret".to_string()),
        ..Settings::default()
    };
    let state = AppState::new(
        settings,
        EngineMetadata::new("kitten_tts_rs", "0.1.0", false),
    );
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/voices")
                .header("xi-api-key", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("X-Request-Id").is_some());
}

#[tokio::test]
async fn protected_route_accepts_bearer_api_key() {
    let settings = Settings {
        auth_enabled: true,
        local_api_key: Some("secret".to_string()),
        ..Settings::default()
    };
    let state = AppState::new(
        settings,
        EngineMetadata::new("kitten_tts_rs", "0.1.0", false),
    );
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/voices")
                .header("Authorization", "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn protected_route_rejects_conflicting_api_key_headers() {
    let settings = Settings {
        auth_enabled: true,
        local_api_key: Some("secret".to_string()),
        ..Settings::default()
    };
    let state = AppState::new(
        settings,
        EngineMetadata::new("kitten_tts_rs", "0.1.0", false),
    );
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/voices")
                .header("xi-api-key", "secret-a")
                .header("Authorization", "Bearer secret-b")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body_json["error"]["message"], "Conflicting API key headers");
}

#[tokio::test]
async fn openai_route_rejects_missing_api_key_with_openai_error_shape() {
    let settings = Settings {
        auth_enabled: true,
        local_api_key: Some("secret".to_string()),
        ..Settings::default()
    };
    let state = AppState::new(
        settings,
        EngineMetadata::new("kitten_tts_rs", "0.1.0", false),
    );
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/audio/speech")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get("X-Request-Id").is_some());

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body_json["error"]["code"], "authentication_failed");
    assert_eq!(body_json["error"]["type"], "invalid_request_error");
    assert_eq!(body_json["error"]["message"], "Missing or invalid API key");
}
