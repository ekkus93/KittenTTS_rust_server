// The server keeps a local path dependency on the sibling `kitten_tts_rs` repo
// and carries compatibility-sensitive fixes there when required, rather than
// reimplementing ONNX/session internals in the HTTP layer.
#[cfg(feature = "real-backend")]
use crate::error::{AppError, AppErrorCode};
#[cfg(feature = "real-backend")]
use crate::models::internal::InternalSynthesisRequest;
#[cfg(feature = "real-backend")]
use crate::services::synth::{
    FloatAudioBuffer, SynthResult, Synthesizer, KITTEN_TTS_CHANNELS, KITTEN_TTS_SAMPLE_RATE,
};
#[cfg(feature = "real-backend")]
use axum::http::StatusCode;
#[cfg(feature = "real-backend")]
use kitten_tts::model::KittenTTS;
#[cfg(feature = "real-backend")]
use serde::Deserialize;
#[cfg(feature = "real-backend")]
use std::fs;
#[cfg(feature = "real-backend")]
use std::path::Path;
#[cfg(feature = "real-backend")]
use std::process::Command;
#[cfg(feature = "real-backend")]
use std::sync::Mutex;
#[cfg(feature = "real-backend")]
type LoadBackendFn = fn(&Path) -> Result<KittenBackend, AppError>;
#[cfg(feature = "real-backend")]
type VerifyEspeakFn = fn() -> Result<(), AppError>;
#[cfg(feature = "real-backend")]
type BackendListVoicesFn = fn(&KittenBackend) -> Vec<String>;
#[cfg(feature = "real-backend")]
type BackendSynthesizeFn =
    fn(&KittenBackend, &InternalSynthesisRequest) -> Result<SynthResult, AppError>;

#[cfg(feature = "real-backend")]
pub(crate) struct KittenBackend {
    inner: Mutex<KittenTTS>,
    available_voices: Vec<String>,
}

#[cfg(feature = "real-backend")]
#[derive(Deserialize)]
struct ModelConfig {
    model_file: String,
    voices: String,
}

#[cfg(feature = "real-backend")]
impl KittenBackend {
    pub(crate) fn from_model_dir(model_dir: &Path) -> Result<Self, AppError> {
        verify_model_dir(model_dir)?;
        verify_espeak_ng()?;

        let model = KittenTTS::from_dir(model_dir).map_err(|err| {
            AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                AppErrorCode::BackendUnavailable,
                format!("failed to initialize kitten_tts backend: {err}"),
            )
        })?;
        let available_voices = model
            .available_voices()
            .into_iter()
            .map(str::to_string)
            .collect();

        Ok(Self {
            inner: Mutex::new(model),
            available_voices,
        })
    }
}

#[cfg(feature = "real-backend")]
impl Synthesizer for KittenBackend {
    fn list_voices(&self) -> Vec<String> {
        self.available_voices.clone()
    }

    fn synthesize(&self, request: &InternalSynthesisRequest) -> Result<SynthResult, AppError> {
        let voice = request
            .voice_id
            .as_ref()
            .ok_or_else(|| AppError::validation("voice_id is required before backend synthesis"))?;

        let mut backend = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("kitten_tts backend mutex poisoned"))?;
        let waveform = backend
            .generate(&request.text, voice, request.speed, false)
            .map_err(|err| {
                AppError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AppErrorCode::Internal,
                    format!("speech synthesis failed: {err}"),
                )
            })?;

        Ok(SynthResult {
            audio: FloatAudioBuffer {
                waveform,
                sample_rate: KITTEN_TTS_SAMPLE_RATE,
                channels: KITTEN_TTS_CHANNELS,
            },
            voice: voice.clone(),
        })
    }
}

#[cfg(feature = "real-backend")]
fn verify_model_dir(model_dir: &Path) -> Result<(), AppError> {
    let config_path = model_dir.join("config.json");
    if !config_path.is_file() {
        return Err(missing_backend_asset(&config_path));
    }

    let config_contents = fs::read_to_string(&config_path).map_err(|err| {
        AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            AppErrorCode::BackendUnavailable,
            format!("failed to read {}: {err}", config_path.display()),
        )
    })?;
    let config: ModelConfig = serde_json::from_str(&config_contents).map_err(|err| {
        AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            AppErrorCode::BackendUnavailable,
            format!("invalid model config {}: {err}", config_path.display()),
        )
    })?;

    let model_path = model_dir.join(config.model_file);
    if !model_path.is_file() {
        return Err(missing_backend_asset(&model_path));
    }

    let voices_path = model_dir.join(config.voices);
    if !voices_path.is_file() {
        return Err(missing_backend_asset(&voices_path));
    }

    Ok(())
}

#[cfg(feature = "real-backend")]
fn missing_backend_asset(path: &Path) -> AppError {
    AppError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        AppErrorCode::BackendUnavailable,
        format!("required backend asset is missing: {}", path.display()),
    )
}

#[cfg(feature = "real-backend")]
pub(crate) fn verify_espeak_ng() -> Result<(), AppError> {
    let status = Command::new("espeak-ng")
        .arg("--version")
        .status()
        .map_err(|err| {
            AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                AppErrorCode::BackendUnavailable,
                format!("failed to execute espeak-ng: {err}"),
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            AppErrorCode::BackendUnavailable,
            "espeak-ng is not available",
        ))
    }
}

#[cfg(feature = "real-backend")]
const _: LoadBackendFn = KittenBackend::from_model_dir;
#[cfg(feature = "real-backend")]
const _: VerifyEspeakFn = verify_espeak_ng;
#[cfg(feature = "real-backend")]
const _: BackendListVoicesFn = <KittenBackend as Synthesizer>::list_voices;
#[cfg(feature = "real-backend")]
const _: BackendSynthesizeFn = <KittenBackend as Synthesizer>::synthesize;

#[cfg(all(test, feature = "real-backend"))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_model_dir(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("kittentts-server-rs-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn verify_model_dir_rejects_missing_config() {
        let model_dir = temp_model_dir("missing-config");
        let error = verify_model_dir(&model_dir).unwrap_err();

        assert_eq!(error.code, AppErrorCode::BackendUnavailable);
        assert!(error.message.contains("config.json"));

        fs::remove_dir_all(model_dir).unwrap();
    }

    #[test]
    fn verify_model_dir_rejects_missing_assets_from_config() {
        let model_dir = temp_model_dir("missing-assets");
        fs::write(
            model_dir.join("config.json"),
            r#"{"model_file":"model.onnx","voices":"voices.npz"}"#,
        )
        .unwrap();

        let error = verify_model_dir(&model_dir).unwrap_err();

        assert_eq!(error.code, AppErrorCode::BackendUnavailable);
        assert!(error.message.contains("model.onnx") || error.message.contains("voices.npz"));

        fs::remove_dir_all(model_dir).unwrap();
    }
}
