use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use kittentts_server_rs::{build_router, AppState, EngineMetadata, Settings};
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
