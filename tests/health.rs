use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use kittentts_server_rs::{build_router, AppState, EngineMetadata, Settings};
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;

fn auth_enabled_state() -> AppState {
    let settings = Settings {
        auth_enabled: true,
        local_api_key: Some("secret".to_string()),
        ..Settings::default()
    };

    AppState::new(
        settings,
        EngineMetadata::new("kitten_tts_rs", "0.1.0", false),
    )
}

fn tts_test_state(settings: Settings) -> AppState {
    AppState::new_test_synth(
        settings,
        vec!["Jasper".to_string(), "Bella".to_string()],
        vec![0.0, 0.25, -0.25, 0.5, -0.5, 0.75],
        24_000,
        1,
    )
}

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
    let state = auth_enabled_state();
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
    let state = auth_enabled_state();
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
    let state = auth_enabled_state();
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
    let state = auth_enabled_state();
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
    let state = auth_enabled_state();
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
    let state = auth_enabled_state();
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

#[tokio::test]
async fn protected_route_accepts_matching_dual_api_key_headers() {
    let state = auth_enabled_state();
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/voices")
                .header("xi-api-key", "secret")
                .header("Authorization", "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("X-Request-Id").is_some());
}

#[tokio::test]
async fn protected_route_rejects_blank_api_key_headers() {
    let state = auth_enabled_state();
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/voices")
                .header("xi-api-key", "   ")
                .header("Authorization", "Bearer    ")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body_json["error"]["message"],
        "Missing or invalid xi-api-key"
    );
}

#[tokio::test]
async fn protected_route_rejects_non_bearer_authorization_header() {
    let state = auth_enabled_state();
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/voices")
                .header("Authorization", "Basic secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body_json["error"]["message"],
        "Missing or invalid xi-api-key"
    );
}

#[tokio::test]
async fn openai_route_rejects_conflicting_api_key_headers_with_openai_error_shape() {
    let state = auth_enabled_state();
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/audio/speech")
                .header("xi-api-key", "secret-a")
                .header("Authorization", "Bearer secret-b")
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
    assert_eq!(body_json["error"]["message"], "Conflicting API key headers");
}

#[tokio::test]
async fn non_v1_unknown_path_is_not_auth_protected_and_returns_not_found() {
    let state = auth_enabled_state();
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz/extra")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(response.headers().get("X-Request-Id").is_some());
}

#[tokio::test]
async fn text_to_speech_response_sets_wav_headers_and_content_length() {
    let app = build_router(tts_test_state(Settings::default()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/text-to-speech")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"text":"hello headers"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "audio/wav");
    assert_eq!(response.headers()["x-output-format"], "wav");
    assert!(response.headers().get("content-length").is_some());
    assert!(response.headers().get("X-Request-Id").is_some());
}

#[tokio::test]
async fn openai_pcm_response_sets_pcm_headers_and_content_length() {
    let app = build_router(tts_test_state(Settings::default()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/audio/speech")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"tts-1","voice":"Bella","input":"hello pcm","response_format":"pcm"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "audio/pcm");
    assert_eq!(response.headers()["x-output-format"], "pcm");
    assert!(response.headers().get("content-length").is_some());
    assert!(response.headers().get("X-Request-Id").is_some());
}

#[tokio::test]
async fn stream_tts_response_omits_content_length_and_keeps_stream_headers() {
    let app = build_router(tts_test_state(Settings::default()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/text-to-speech/Bella/stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"text":"hello stream headers","output_format":"pcm_16000"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "audio/pcm");
    assert_eq!(response.headers()["x-output-format"], "pcm_16000");
    assert!(response.headers().get("content-length").is_none());
    assert!(response.headers().get("X-Request-Id").is_some());
}

#[tokio::test]
async fn non_strict_text_to_speech_invalid_output_format_falls_back_to_wav() {
    let settings = Settings {
        strict_mode: false,
        output_format: "wav".to_string(),
        ..Settings::default()
    };
    let app = build_router(tts_test_state(settings));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/text-to-speech")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"text":"hello fallback","output_format":"mp3"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "audio/wav");
    assert_eq!(response.headers()["x-output-format"], "wav");
    assert!(response.headers().get("content-length").is_some());
}

#[tokio::test]
async fn non_strict_stream_invalid_output_format_falls_back_to_wav() {
    let settings = Settings {
        strict_mode: false,
        output_format: "wav".to_string(),
        ..Settings::default()
    };
    let app = build_router(tts_test_state(settings));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/text-to-speech/Bella/stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"text":"hello fallback stream","output_format":"mp3"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "audio/wav");
    assert_eq!(response.headers()["x-output-format"], "wav");
    assert!(response.headers().get("content-length").is_none());
}

#[tokio::test]
async fn strict_text_to_speech_invalid_output_format_returns_validation_error() {
    let settings = Settings {
        strict_mode: true,
        ..Settings::default()
    };
    let app = build_router(tts_test_state(settings));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/text-to-speech")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"text":"hello strict","output_format":"mp3"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response.headers().get("X-Request-Id").is_some());

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body_json["error"]["code"], "validation_error");
    assert!(body_json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Unsupported output_format"));
}

#[tokio::test]
async fn strict_stream_invalid_output_format_returns_validation_error() {
    let settings = Settings {
        strict_mode: true,
        ..Settings::default()
    };
    let app = build_router(tts_test_state(settings));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/text-to-speech/Bella/stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"text":"hello strict stream","output_format":"mp3"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response.headers().get("X-Request-Id").is_some());

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body_json["error"]["code"], "validation_error");
    assert!(body_json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Unsupported output_format"));
}
