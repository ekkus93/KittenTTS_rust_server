use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::app_state::AppState;
use crate::models::api::HealthResponse;

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/healthz", get(healthz))
}

async fn healthz(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        engine: state.engine_metadata.engine,
        engine_version: state.engine_metadata.engine_version,
        model_loaded: state.synth_runtime.model_loaded(),
        onnx_runtime_source: state.engine_metadata.onnx_runtime_source,
        onnx_runtime_path: state
            .engine_metadata
            .onnx_runtime_path
            .map(|path| path.display().to_string()),
        default_voice_id: state.settings.default_voice_id,
        output_format: state.settings.output_format,
        sample_rate: state.settings.sample_rate,
        channel_layout: state.settings.channel_layout,
    })
}
