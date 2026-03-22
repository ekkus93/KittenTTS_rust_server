use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::app_state::AppState;
use crate::models::api::VoiceListResponse;
use crate::services::voices::build_voice_descriptors;

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/v1/voices", get(list_voices))
}

async fn list_voices(State(state): State<AppState>) -> Json<VoiceListResponse> {
    let available_voices = state.synth_runtime.synthesizer().list_voices();
    Json(VoiceListResponse {
        voices: build_voice_descriptors(&available_voices, &state.settings.voice_map),
    })
}

#[cfg(test)]
mod tests {
    use crate::app_state::AppState;
    use crate::config::Settings;
    use crate::error::AppError;
    use crate::models::internal::InternalSynthesisRequest;
    use crate::services::synth::{test_runtime, SynthResult, Synthesizer};
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use std::collections::BTreeMap;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct FakeSynthesizer {
        available_voices: Vec<String>,
    }

    impl Synthesizer for FakeSynthesizer {
        fn list_voices(&self) -> Vec<String> {
            self.available_voices.clone()
        }

        fn synthesize(&self, _request: &InternalSynthesisRequest) -> Result<SynthResult, AppError> {
            panic!("synthesize should not be called in voices route tests")
        }
    }

    fn test_state() -> AppState {
        let settings = Settings {
            voice_map: BTreeMap::from([
                ("Narrator".to_string(), "Bella".to_string()),
                ("Storyteller".to_string(), "Bella".to_string()),
            ]),
            ..Settings::default()
        };

        let synthesizer = FakeSynthesizer {
            available_voices: vec!["Jasper".to_string(), "Bella".to_string()],
        };

        AppState::from_runtime(settings, test_runtime(synthesizer))
    }

    #[tokio::test]
    async fn list_voices_returns_voice_descriptors() {
        let app = crate::routes::build_router(test_state());

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

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        let voices = payload["voices"].as_array().unwrap();

        assert_eq!(voices.len(), 2);
        assert_eq!(voices[0]["voice_id"], "bella");
        assert_eq!(voices[0]["name"], "Bella");
        assert_eq!(voices[0]["category"], "premade");
        assert_eq!(voices[0]["available_for_tiers"][0], "local");
        assert_eq!(voices[0]["labels"]["provider"], "KittenTTS");
        assert_eq!(voices[0]["labels"]["source"], "local");
        assert_eq!(voices[0]["labels"]["kitten_voice"], "Bella");
        assert_eq!(voices[0]["labels"]["aliases"][0], "Narrator");
        assert_eq!(voices[0]["labels"]["aliases"][1], "Storyteller");
        assert!(voices[0]["description"]
            .as_str()
            .unwrap()
            .contains("Also reachable via aliases"));
        assert_eq!(voices[1]["voice_id"], "jasper");
        assert_eq!(voices[1]["name"], "Jasper");
    }
}
