// The server keeps a local path dependency on the sibling `kitten_tts_rs` repo
// and carries compatibility-sensitive fixes there when required, rather than
// reimplementing ONNX/session internals in the HTTP layer.
#[cfg(feature = "real-backend")]
use crate::config::Settings;
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
use kitten_tts::model::{KittenTTS, DEFAULT_MODEL_REPO};
#[cfg(feature = "real-backend")]
use serde::Deserialize;
#[cfg(feature = "real-backend")]
use std::env;
#[cfg(feature = "real-backend")]
use std::fs;
#[cfg(feature = "real-backend")]
use std::path::{Path, PathBuf};
#[cfg(feature = "real-backend")]
use std::process::Command;
#[cfg(feature = "real-backend")]
use std::sync::Mutex;
#[cfg(feature = "real-backend")]
use tracing::info;
#[cfg(feature = "real-backend")]
type LoadBackendFn = fn(&Path) -> Result<KittenBackend, AppError>;
#[cfg(feature = "real-backend")]
type VerifyEspeakFn = fn() -> Result<(), AppError>;
#[cfg(feature = "real-backend")]
type VerifyEspeakBinaryFn = fn(&str) -> Result<(), AppError>;
#[cfg(feature = "real-backend")]
type BackendListVoicesFn = fn(&KittenBackend) -> Vec<String>;
#[cfg(feature = "real-backend")]
type BackendSynthesizeFn =
    fn(&KittenBackend, &InternalSynthesisRequest) -> Result<SynthResult, AppError>;

#[cfg(feature = "real-backend")]
const ORT_DYLIB_ENV: &str = "ORT_DYLIB_PATH";

#[cfg(feature = "real-backend")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OrtRuntimeMetadata {
    pub(crate) source: &'static str,
    pub(crate) path: Option<PathBuf>,
}

#[cfg(feature = "real-backend")]
enum OrtDylibSource {
    Configured(PathBuf),
    LocalDiscovery(PathBuf),
    SystemDefault,
}

#[cfg(feature = "real-backend")]
enum BackendModelSource<'a> {
    ModelDir(&'a Path),
    HuggingFaceRepo(&'a str),
}

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
    pub(crate) fn from_settings(
        settings: &Settings,
    ) -> Result<(Self, OrtRuntimeMetadata), AppError> {
        let ort_runtime = configure_default_ort_dylib_path();

        let backend = match backend_model_source(settings) {
            BackendModelSource::ModelDir(model_dir) => Self::from_model_dir(model_dir),
            BackendModelSource::HuggingFaceRepo(repo_id) => Self::from_repo(repo_id),
        }?;

        Ok((backend, ort_runtime))
    }

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

    pub(crate) fn from_repo(repo_id: &str) -> Result<Self, AppError> {
        verify_espeak_ng()?;

        let model = KittenTTS::from_repo(repo_id, None).map_err(|err| {
            AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                AppErrorCode::BackendUnavailable,
                format!("failed to download or initialize kitten_tts backend for {repo_id}: {err}"),
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
fn backend_model_source(settings: &Settings) -> BackendModelSource<'_> {
    if let Some(model_dir) = settings.model_dir.as_deref() {
        BackendModelSource::ModelDir(model_dir)
    } else {
        BackendModelSource::HuggingFaceRepo(DEFAULT_MODEL_REPO)
    }
}

#[cfg(feature = "real-backend")]
fn configure_default_ort_dylib_path() -> OrtRuntimeMetadata {
    match selected_ort_dylib_source() {
        OrtDylibSource::Configured(path) => {
            info!(source = "env", path = %path.display(), "selected ONNX Runtime shared library path");
            OrtRuntimeMetadata {
                source: "env",
                path: Some(path),
            }
        }
        OrtDylibSource::LocalDiscovery(path) => {
            info!(source = "local_discovery", path = %path.display(), "selected ONNX Runtime shared library path");
            env::set_var(ORT_DYLIB_ENV, &path);
            OrtRuntimeMetadata {
                source: "local_discovery",
                path: Some(path),
            }
        }
        OrtDylibSource::SystemDefault => {
            info!(
                source = "system_default",
                "selected ONNX Runtime shared library path"
            );
            OrtRuntimeMetadata {
                source: "system_default",
                path: None,
            }
        }
    }
}

#[cfg(feature = "real-backend")]
fn selected_ort_dylib_source() -> OrtDylibSource {
    if let Some(path) = env::var_os(ORT_DYLIB_ENV) {
        return OrtDylibSource::Configured(PathBuf::from(path));
    }

    if let Some(path) = discover_default_ort_dylib_path() {
        return OrtDylibSource::LocalDiscovery(path);
    }

    OrtDylibSource::SystemDefault
}

#[cfg(feature = "real-backend")]
fn discover_default_ort_dylib_path() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    let root = PathBuf::from(home).join(".local/share/onnxruntime");
    let root_entries = fs::read_dir(root).ok()?;

    let mut candidates: Vec<PathBuf> = root_entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|dir| newest_ort_dylib_in_dir(&dir))
        .collect();

    candidates.sort_by(|left, right| right.cmp(left));
    candidates.into_iter().next()
}

#[cfg(feature = "real-backend")]
fn newest_ort_dylib_in_dir(dir: &Path) -> Option<PathBuf> {
    let dir_entries = fs::read_dir(dir).ok()?;
    let mut dylibs: Vec<PathBuf> = dir_entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("libonnxruntime.so") && path.exists() && path.is_file()
                })
        })
        .collect();

    dylibs.sort_by(|left, right| right.cmp(left));
    dylibs.into_iter().next()
}

#[cfg(feature = "real-backend")]
pub(crate) fn verify_espeak_ng() -> Result<(), AppError> {
    verify_espeak_ng_binary("espeak-ng")
}

#[cfg(feature = "real-backend")]
fn verify_espeak_ng_binary(program: &str) -> Result<(), AppError> {
    let status = Command::new(program)
        .arg("--version")
        .status()
        .map_err(|err| {
            AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                AppErrorCode::BackendUnavailable,
                format!("failed to execute {program}: {err}"),
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            AppErrorCode::BackendUnavailable,
            format!("{program} is not available"),
        ))
    }
}

#[cfg(feature = "real-backend")]
const _: fn(&Settings) -> Result<(KittenBackend, OrtRuntimeMetadata), AppError> =
    KittenBackend::from_settings;
#[cfg(feature = "real-backend")]
const _: LoadBackendFn = KittenBackend::from_model_dir;
#[cfg(feature = "real-backend")]
const _: fn(&str) -> Result<KittenBackend, AppError> = KittenBackend::from_repo;
#[cfg(feature = "real-backend")]
const _: fn() -> OrtRuntimeMetadata = configure_default_ort_dylib_path;
#[cfg(feature = "real-backend")]
const _: fn() -> OrtDylibSource = selected_ort_dylib_source;
#[cfg(feature = "real-backend")]
const _: fn() -> Option<PathBuf> = discover_default_ort_dylib_path;
#[cfg(feature = "real-backend")]
const _: VerifyEspeakFn = verify_espeak_ng;
#[cfg(feature = "real-backend")]
const _: VerifyEspeakBinaryFn = verify_espeak_ng_binary;
#[cfg(feature = "real-backend")]
const _: BackendListVoicesFn = <KittenBackend as Synthesizer>::list_voices;
#[cfg(feature = "real-backend")]
const _: BackendSynthesizeFn = <KittenBackend as Synthesizer>::synthesize;

#[cfg(all(test, feature = "real-backend"))]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::services::synth::create_synth_runtime;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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

    #[test]
    fn verify_espeak_ng_rejects_missing_executable() {
        let error = verify_espeak_ng_binary("definitely-not-a-real-espeak-ng-binary").unwrap_err();

        assert_eq!(error.code, AppErrorCode::BackendUnavailable);
        assert!(error.message.contains("failed to execute"));
    }

    #[test]
    fn backend_model_source_prefers_explicit_model_dir() {
        let settings = Settings {
            model_dir: Some(PathBuf::from("/srv/kitten/model")),
            ..Settings::default()
        };

        match backend_model_source(&settings) {
            BackendModelSource::ModelDir(path) => {
                assert_eq!(path, Path::new("/srv/kitten/model"));
            }
            BackendModelSource::HuggingFaceRepo(_) => panic!("expected explicit model_dir source"),
        }
    }

    #[test]
    fn backend_model_source_defaults_to_python_repo() {
        let settings = Settings::default();

        match backend_model_source(&settings) {
            BackendModelSource::ModelDir(_) => panic!("expected Hugging Face repo fallback"),
            BackendModelSource::HuggingFaceRepo(repo_id) => {
                assert_eq!(repo_id, DEFAULT_MODEL_REPO);
            }
        }
    }

    #[test]
    fn discover_default_ort_dylib_path_prefers_local_shared_library() {
        let _guard = env_guard();
        let home_dir = temp_model_dir("ort-home");
        let ort_dir = home_dir.join(".local/share/onnxruntime/1.24.2");
        fs::create_dir_all(&ort_dir).unwrap();
        let expected_path = ort_dir.join("libonnxruntime.so.1.24.2");
        fs::write(&expected_path, b"fake-ort").unwrap();
        env::set_var("HOME", &home_dir);

        let discovered = discover_default_ort_dylib_path();

        assert_eq!(discovered.as_deref(), Some(expected_path.as_path()));

        fs::remove_dir_all(home_dir).unwrap();
    }

    #[test]
    fn configure_default_ort_dylib_path_respects_existing_override() {
        let _guard = env_guard();
        env::set_var(ORT_DYLIB_ENV, "/already/configured/libonnxruntime.so");

        let metadata = configure_default_ort_dylib_path();

        assert_eq!(
            env::var_os(ORT_DYLIB_ENV),
            Some("/already/configured/libonnxruntime.so".into())
        );
        assert_eq!(
            metadata,
            OrtRuntimeMetadata {
                source: "env",
                path: Some(PathBuf::from("/already/configured/libonnxruntime.so")),
            }
        );
    }

    #[test]
    fn selected_ort_dylib_source_uses_configured_env_first() {
        let _guard = env_guard();
        env::set_var(ORT_DYLIB_ENV, "/already/configured/libonnxruntime.so");

        match selected_ort_dylib_source() {
            OrtDylibSource::Configured(path) => {
                assert_eq!(path, PathBuf::from("/already/configured/libonnxruntime.so"));
            }
            OrtDylibSource::LocalDiscovery(_) => panic!("expected configured env source"),
            OrtDylibSource::SystemDefault => panic!("expected configured env source"),
        }
    }

    #[test]
    fn configure_default_ort_dylib_path_sets_local_candidate() {
        let _guard = env_guard();
        let home_dir = temp_model_dir("ort-home-configure");
        let ort_dir = home_dir.join(".local/share/onnxruntime/1.24.2");
        fs::create_dir_all(&ort_dir).unwrap();
        let expected_path = ort_dir.join("libonnxruntime.so.1.24.2");
        fs::write(&expected_path, b"fake-ort").unwrap();
        env::set_var("HOME", &home_dir);

        let metadata = configure_default_ort_dylib_path();

        assert_eq!(env::var_os(ORT_DYLIB_ENV), Some(expected_path.into()));
        assert_eq!(
            metadata,
            OrtRuntimeMetadata {
                source: "local_discovery",
                path: Some(ort_dir.join("libonnxruntime.so.1.24.2")),
            }
        );

        fs::remove_dir_all(home_dir).unwrap();
    }

    #[test]
    fn configure_default_ort_dylib_path_reports_system_default_when_nothing_is_found() {
        let _guard = env_guard();
        let home_dir = temp_model_dir("ort-home-system-default-configure");
        env::set_var("HOME", &home_dir);

        let metadata = configure_default_ort_dylib_path();

        assert_eq!(
            metadata,
            OrtRuntimeMetadata {
                source: "system_default",
                path: None,
            }
        );
        assert_eq!(env::var_os(ORT_DYLIB_ENV), None);

        fs::remove_dir_all(home_dir).unwrap();
    }

    #[test]
    fn selected_ort_dylib_source_falls_back_to_system_default() {
        let _guard = env_guard();
        let home_dir = temp_model_dir("ort-home-system-default");
        env::set_var("HOME", &home_dir);

        match selected_ort_dylib_source() {
            OrtDylibSource::SystemDefault => {}
            OrtDylibSource::Configured(_) => panic!("expected system default source"),
            OrtDylibSource::LocalDiscovery(_) => panic!("expected system default source"),
        }

        fs::remove_dir_all(home_dir).unwrap();
    }

    #[test]
    #[ignore = "requires KITTENTTS_SERVER_TEST_MODEL_DIR and a host environment that can link/run the real backend"]
    fn create_synth_runtime_can_generate_speech_with_real_model_assets() {
        let model_dir = std::env::var_os("KITTENTTS_SERVER_TEST_MODEL_DIR")
            .map(PathBuf::from)
            .expect("KITTENTTS_SERVER_TEST_MODEL_DIR must be set for this test");
        let settings = Settings {
            model_dir: Some(model_dir),
            default_voice_id: "jasper".to_string(),
            ..Settings::default()
        };

        let runtime = create_synth_runtime(&settings).expect("runtime should initialize");
        let result = runtime
            .synthesizer()
            .synthesize(&InternalSynthesisRequest {
                text: "Hello from the Rust backend".to_string(),
                voice_id: Some("jasper".to_string()),
                model_id: None,
                speed: 1.0,
                output_format: Some("wav".to_string()),
                streaming: false,
            })
            .expect("synthesis should succeed");

        assert!(runtime.model_loaded());
        assert_eq!(result.audio.sample_rate, KITTEN_TTS_SAMPLE_RATE);
        assert_eq!(result.audio.channels, KITTEN_TTS_CHANNELS);
        assert!(!result.audio.waveform.is_empty());
    }

    struct EnvResetGuard {
        saved_home: Option<std::ffi::OsString>,
        saved_ort_dylib_path: Option<std::ffi::OsString>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for EnvResetGuard {
        fn drop(&mut self) {
            match &self.saved_home {
                Some(value) => env::set_var("HOME", value),
                None => env::remove_var("HOME"),
            }

            match &self.saved_ort_dylib_path {
                Some(value) => env::set_var(ORT_DYLIB_ENV, value),
                None => env::remove_var(ORT_DYLIB_ENV),
            }
        }
    }

    fn env_guard() -> EnvResetGuard {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let saved_home = env::var_os("HOME");
        let saved_ort_dylib_path = env::var_os(ORT_DYLIB_ENV);
        env::remove_var(ORT_DYLIB_ENV);

        EnvResetGuard {
            saved_home,
            saved_ort_dylib_path,
            _lock: lock,
        }
    }
}
