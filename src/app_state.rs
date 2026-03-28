use crate::config::Settings;
use crate::error::AppError;
use crate::services::synth::{create_synth_runtime, unavailable_runtime, SynthRuntime};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct EngineMetadata {
    pub engine: String,
    pub engine_version: Option<String>,
    pub model_loaded: bool,
    pub onnx_runtime_source: Option<String>,
    pub onnx_runtime_path: Option<PathBuf>,
}

impl EngineMetadata {
    pub fn new(
        engine: impl Into<String>,
        engine_version: impl Into<String>,
        model_loaded: bool,
    ) -> Self {
        Self {
            engine: engine.into(),
            engine_version: Some(engine_version.into()),
            model_loaded,
            onnx_runtime_source: None,
            onnx_runtime_path: None,
        }
    }

    pub(crate) fn from_runtime(runtime: &SynthRuntime) -> Self {
        Self {
            engine: runtime.engine_name().to_string(),
            engine_version: runtime.engine_version().map(str::to_string),
            model_loaded: runtime.model_loaded(),
            onnx_runtime_source: runtime.onnx_runtime_source().map(str::to_string),
            onnx_runtime_path: runtime.onnx_runtime_path().cloned(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub engine_metadata: EngineMetadata,
    pub(crate) synth_runtime: SynthRuntime,
}

impl AppState {
    pub fn new(settings: Settings, engine_metadata: EngineMetadata) -> Self {
        let synth_runtime = unavailable_runtime(&settings);

        Self {
            settings,
            engine_metadata,
            synth_runtime,
        }
    }

    pub(crate) fn from_runtime(settings: Settings, synth_runtime: SynthRuntime) -> Self {
        let engine_metadata = EngineMetadata::from_runtime(&synth_runtime);

        Self {
            settings,
            engine_metadata,
            synth_runtime,
        }
    }
}

pub fn initialize_app_state(settings: Settings) -> Result<AppState, AppError> {
    initialize_app_state_with_factory(settings, create_synth_runtime)
}

fn initialize_app_state_with_factory<F>(
    settings: Settings,
    create_runtime: F,
) -> Result<AppState, AppError>
where
    F: FnOnce(&Settings) -> Result<SynthRuntime, AppError>,
{
    let synth_runtime = create_runtime(&settings)?;
    Ok(AppState::from_runtime(settings, synth_runtime))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppErrorCode;
    use crate::models::internal::InternalSynthesisRequest;
    use crate::services::synth::{test_runtime, FloatAudioBuffer, SynthResult, Synthesizer};
    use axum::http::StatusCode;

    struct FakeSynthesizer;

    impl Synthesizer for FakeSynthesizer {
        fn list_voices(&self) -> Vec<String> {
            vec!["jasper".to_string()]
        }

        fn synthesize(&self, request: &InternalSynthesisRequest) -> Result<SynthResult, AppError> {
            Ok(SynthResult {
                audio: FloatAudioBuffer {
                    waveform: vec![0.0, 0.25, -0.25],
                    sample_rate: 24_000,
                    channels: 1,
                },
                voice: request
                    .voice_id
                    .clone()
                    .unwrap_or_else(|| "jasper".to_string()),
            })
        }
    }

    #[test]
    fn initialize_app_state_uses_runtime_metadata_from_factory() {
        let settings = Settings {
            host: "127.0.0.1".to_string(),
            port: 8012,
            default_voice_id: "jasper".to_string(),
            output_format: "wav".to_string(),
            ..Settings::default()
        };
        let expected_settings = settings.clone();

        let state =
            initialize_app_state_with_factory(settings, |_| Ok(test_runtime(FakeSynthesizer)))
                .expect("startup should succeed with a deterministic runtime factory");

        assert_eq!(state.settings, expected_settings);
        assert_eq!(state.engine_metadata.engine, "kitten_tts_rs");
        assert_eq!(state.engine_metadata.engine_version, None);
        assert!(state.engine_metadata.model_loaded);
    }

    #[test]
    fn initialize_app_state_propagates_runtime_factory_errors() {
        let error = match initialize_app_state_with_factory(Settings::default(), |_| {
            Err(AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                AppErrorCode::BackendUnavailable,
                "backend offline",
            ))
        }) {
            Ok(_) => panic!("startup should return the backend initialization error"),
            Err(error) => error,
        };

        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.code, AppErrorCode::BackendUnavailable);
        assert_eq!(error.message, "backend offline");
    }
}
