use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use kittentts_server_rs::{build_router, AppState, EngineMetadata, Settings};
use tower::ServiceExt;

#[tokio::test]
async fn health_route_returns_server_metadata() {
    let state = AppState::new(
        Settings::default(),
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

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_text.contains("\"status\":\"ok\""));
    assert!(body_text.contains("\"default_voice_id\":\"jasper\""));
}
