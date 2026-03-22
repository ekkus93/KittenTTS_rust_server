#[cfg(feature = "real-backend")]
use crate::backend::kitten::KittenBackend;
use crate::config::Settings;
use crate::error::{AppError, AppErrorCode};
use crate::models::internal::InternalSynthesisRequest;
use axum::http::StatusCode;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(feature = "real-backend")]
pub(crate) const KITTEN_TTS_SAMPLE_RATE: u32 = 24_000;
#[cfg(feature = "real-backend")]
pub(crate) const KITTEN_TTS_CHANNELS: u16 = 1;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FloatAudioBuffer {
    pub waveform: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SynthResult {
    pub audio: FloatAudioBuffer,
    pub voice: String,
}

pub(crate) trait Synthesizer: Send + Sync {
    fn list_voices(&self) -> Vec<String>;

    fn synthesize(&self, request: &InternalSynthesisRequest) -> Result<SynthResult, AppError>;
}

#[derive(Clone)]
pub(crate) struct SynthRuntime {
    synthesizer: Arc<dyn Synthesizer>,
    engine_name: String,
    engine_version: Option<String>,
    model_loaded: bool,
    onnx_runtime_source: Option<String>,
    onnx_runtime_path: Option<PathBuf>,
}

impl SynthRuntime {
    pub(crate) fn synthesizer(&self) -> Arc<dyn Synthesizer> {
        Arc::clone(&self.synthesizer)
    }

    pub(crate) fn engine_name(&self) -> &str {
        &self.engine_name
    }

    pub(crate) fn engine_version(&self) -> Option<&str> {
        self.engine_version.as_deref()
    }

    pub(crate) fn model_loaded(&self) -> bool {
        self.model_loaded
    }

    pub(crate) fn onnx_runtime_source(&self) -> Option<&str> {
        self.onnx_runtime_source.as_deref()
    }

    pub(crate) fn onnx_runtime_path(&self) -> Option<&PathBuf> {
        self.onnx_runtime_path.as_ref()
    }
}

#[cfg(test)]
pub(crate) fn test_runtime<S>(synthesizer: S) -> SynthRuntime
where
    S: Synthesizer + 'static,
{
    SynthRuntime {
        synthesizer: Arc::new(synthesizer),
        engine_name: "kitten_tts_rs".to_string(),
        engine_version: None,
        model_loaded: true,
        onnx_runtime_source: None,
        onnx_runtime_path: None,
    }
}

#[derive(Clone, Debug)]
pub(crate) struct UnavailableSynthesizer {
    reason: String,
    available_voices: Vec<String>,
}

impl UnavailableSynthesizer {
    pub(crate) fn new(reason: impl Into<String>, available_voices: Vec<String>) -> Self {
        Self {
            reason: reason.into(),
            available_voices,
        }
    }
}

impl Synthesizer for UnavailableSynthesizer {
    fn list_voices(&self) -> Vec<String> {
        self.available_voices.clone()
    }

    fn synthesize(&self, _request: &InternalSynthesisRequest) -> Result<SynthResult, AppError> {
        Err(AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            AppErrorCode::BackendUnavailable,
            self.reason.clone(),
        ))
    }
}

pub(crate) fn discover_stub_voices(settings: &Settings) -> Vec<String> {
    let mut voices: BTreeSet<String> = BTreeSet::new();
    if !settings.default_voice_id.is_empty() {
        voices.insert(settings.default_voice_id.clone());
    }
    voices.extend(settings.voice_map.values().cloned());
    voices.into_iter().collect()
}

pub(crate) fn unavailable_runtime(settings: &Settings) -> SynthRuntime {
    SynthRuntime {
        synthesizer: Arc::new(UnavailableSynthesizer::new(
            "synthesis backend is not available",
            discover_stub_voices(settings),
        )),
        engine_name: "kitten_tts_rs".to_string(),
        engine_version: None,
        model_loaded: false,
        onnx_runtime_source: None,
        onnx_runtime_path: None,
    }
}

pub(crate) fn create_synth_runtime(settings: &Settings) -> Result<SynthRuntime, AppError> {
    #[cfg(feature = "real-backend")]
    {
        let backend = KittenBackend::from_settings(settings)?;
        let ort_metadata = crate::backend::kitten::cached_ort_metadata();
        Ok(SynthRuntime {
            synthesizer: Arc::new(backend),
            engine_name: "kitten_tts_rs".to_string(),
            engine_version: None,
            model_loaded: true,
            onnx_runtime_source: ort_metadata.map(|m| m.source.to_string()),
            onnx_runtime_path: ort_metadata.and_then(|m| m.path.clone()),
        })
    }

    #[cfg(not(feature = "real-backend"))]
    {
        let _ = settings;
        Err(AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            AppErrorCode::BackendUnavailable,
            "server was built without the real-backend feature; rebuild with `--features real-backend`",
        ))
    }
}

type DiscoverStubVoicesFn = fn(&Settings) -> Vec<String>;
type UnavailableRuntimeFn = fn(&Settings) -> SynthRuntime;
type CreateRuntimeFn = fn(&Settings) -> Result<SynthRuntime, AppError>;
type RuntimeSynthesizerFn = fn(&SynthRuntime) -> Arc<dyn Synthesizer>;
type RuntimeEngineNameFn = fn(&SynthRuntime) -> &str;
type RuntimeEngineVersionFn = fn(&SynthRuntime) -> Option<&str>;
type RuntimeModelLoadedFn = fn(&SynthRuntime) -> bool;
type RuntimeOnnxSourceFn = fn(&SynthRuntime) -> Option<&str>;
type RuntimeOnnxPathFn = fn(&SynthRuntime) -> Option<&PathBuf>;
type UnavailableListVoicesFn = fn(&UnavailableSynthesizer) -> Vec<String>;
type UnavailableSynthesizeFn =
    fn(&UnavailableSynthesizer, &InternalSynthesisRequest) -> Result<SynthResult, AppError>;
#[cfg(test)]
type TestRuntimeFn = fn(UnavailableSynthesizer) -> SynthRuntime;

// Compile-time signature checks: these `const _:` bindings verify that each
// crate-private function and method still matches its declared type alias.
// They produce a type error at compile time if a signature drifts from the
// alias, acting as a lightweight contract test with no runtime cost.
const _: Option<FloatAudioBuffer> = None;
const _: Option<SynthResult> = None;
const _: Option<SynthRuntime> = None;
const _: Option<UnavailableSynthesizer> = None;
const _: DiscoverStubVoicesFn = discover_stub_voices;
const _: UnavailableRuntimeFn = unavailable_runtime;
const _: CreateRuntimeFn = create_synth_runtime;
const _: RuntimeSynthesizerFn = SynthRuntime::synthesizer;
const _: RuntimeEngineNameFn = SynthRuntime::engine_name;
const _: RuntimeEngineVersionFn = SynthRuntime::engine_version;
const _: RuntimeModelLoadedFn = SynthRuntime::model_loaded;
const _: RuntimeOnnxSourceFn = SynthRuntime::onnx_runtime_source;
const _: RuntimeOnnxPathFn = SynthRuntime::onnx_runtime_path;
const _: UnavailableListVoicesFn = <UnavailableSynthesizer as Synthesizer>::list_voices;
const _: UnavailableSynthesizeFn = <UnavailableSynthesizer as Synthesizer>::synthesize;
#[cfg(test)]
const _: TestRuntimeFn = test_runtime::<UnavailableSynthesizer>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    #[test]
    fn discover_stub_voices_uses_default_and_alias_targets() {
        let settings = Settings {
            default_voice_id: "jasper".to_string(),
            voice_map: [("Narrator".to_string(), "hugo".to_string())]
                .into_iter()
                .collect(),
            ..Settings::default()
        };

        let voices = discover_stub_voices(&settings);

        assert_eq!(voices, vec!["hugo".to_string(), "jasper".to_string()]);
    }

    #[test]
    fn unavailable_synthesizer_returns_backend_unavailable() {
        let synthesizer =
            UnavailableSynthesizer::new("backend offline", vec!["jasper".to_string()]);
        let request = InternalSynthesisRequest {
            text: "hello".to_string(),
            voice_id: Some("jasper".to_string()),
            model_id: None,
            speed: 1.0,
            output_format: Some("wav".to_string()),
            streaming: false,
        };

        let error = synthesizer.synthesize(&request).unwrap_err();

        assert_eq!(synthesizer.list_voices(), vec!["jasper".to_string()]);
        assert_eq!(error.code, AppErrorCode::BackendUnavailable);
        assert_eq!(error.message, "backend offline");
    }

    #[cfg(not(feature = "real-backend"))]
    #[test]
    fn create_synth_runtime_fails_fast_without_real_backend_feature() {
        let error = match create_synth_runtime(&Settings::default()) {
            Ok(_) => panic!("expected fail-fast startup error without real-backend feature"),
            Err(error) => error,
        };

        assert_eq!(error.code, AppErrorCode::BackendUnavailable);
        assert!(error.message.contains("real-backend"));
    }
}
