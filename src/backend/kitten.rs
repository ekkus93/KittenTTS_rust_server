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
use std::sync::{Mutex, OnceLock};
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
static ORT_PRE_RUNTIME_METADATA: OnceLock<OrtRuntimeMetadata> = OnceLock::new();

/// Resolves the ONNX Runtime shared library path and records the result in a
/// process-wide cache.  Must be called **before** the tokio runtime starts so
/// that `env::set_var` is not racing with other threads.
#[cfg(feature = "real-backend")]
pub(crate) fn setup_ort_before_runtime() {
    let metadata = apply_ort_dylib_path();
    // OnceLock::set is intentionally allowed to fail on repeated calls (e.g.
    // in tests that bypass the normal startup path).
    let _ = ORT_PRE_RUNTIME_METADATA.set(metadata);
}

/// Returns the ONNX Runtime metadata recorded by `setup_ort_before_runtime`,
/// or `None` if that function was not called before the backend was created.
#[cfg(feature = "real-backend")]
pub(crate) fn cached_ort_metadata() -> Option<&'static OrtRuntimeMetadata> {
    ORT_PRE_RUNTIME_METADATA.get()
}

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
    pub(crate) fn from_settings(settings: &Settings) -> Result<Self, AppError> {
        if let Some(metadata) = cached_ort_metadata() {
            info!(
                source = metadata.source,
                path = ?metadata.path,
                "ONNX Runtime shared library path"
            );
        }

        match backend_model_source(settings) {
            BackendModelSource::ModelDir(model_dir) => Self::from_model_dir(model_dir),
            BackendModelSource::HuggingFaceRepo(repo_id) => Self::from_repo(repo_id),
        }
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
        let mut backend = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("kitten_tts backend mutex poisoned"))?;
        let voice = request
            .voice_id
            .as_ref()
            .ok_or_else(|| AppError::validation("voice_id is required before backend synthesis"))?;
        let waveform = generate_waveform_with_clean_text(
            request,
            |text, requested_voice, speed, clean_text| {
                backend.generate(text, requested_voice, speed, clean_text)
            },
        )?;

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
fn generate_waveform_with_clean_text<F, E>(
    request: &InternalSynthesisRequest,
    mut generate: F,
) -> Result<Vec<f32>, AppError>
where
    F: FnMut(&str, &str, f32, bool) -> Result<Vec<f32>, E>,
    E: std::fmt::Display,
{
    let voice = request
        .voice_id
        .as_ref()
        .ok_or_else(|| AppError::validation("voice_id is required before backend synthesis"))?;

    // The HTTP compatibility layer intentionally forces clean_text off to match
    // the Python server's backend call path.
    generate(&request.text, voice, request.speed, false).map_err(|err| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            AppErrorCode::Internal,
            format!("speech synthesis failed: {err}"),
        )
    })
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

/// Parse a dotted version string like `"1.24.2"` into a `(major, minor, patch)` tuple
/// so version numbers are compared numerically rather than lexicographically.
/// Unparseable components default to `0`.
#[cfg(feature = "real-backend")]
fn parse_semver(s: &str) -> (u32, u32, u32) {
    let mut parts = s.splitn(4, '.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

#[cfg(feature = "real-backend")]
fn apply_ort_dylib_path() -> OrtRuntimeMetadata {
    match selected_ort_dylib_source() {
        OrtDylibSource::Configured(path) => OrtRuntimeMetadata {
            source: "env",
            path: Some(path),
        },
        OrtDylibSource::LocalDiscovery(path) => {
            // SAFETY: this function is called before the tokio runtime starts
            // (via `setup_ort_before_runtime` in `main`), so the process is
            // single-threaded here and no other thread can concurrently read or
            // write environment variables.
            #[allow(unused_unsafe)]
            unsafe {
                env::set_var(ORT_DYLIB_ENV, &path)
            };
            OrtRuntimeMetadata {
                source: "local_discovery",
                path: Some(path),
            }
        }
        OrtDylibSource::SystemDefault => OrtRuntimeMetadata {
            source: "system_default",
            path: None,
        },
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

    candidates.sort_by(|left, right| {
        let semver_key = |p: &PathBuf| {
            p.parent()
                .and_then(|dir| dir.file_name())
                .and_then(|name| name.to_str())
                .map(parse_semver)
                .unwrap_or_default()
        };
        semver_key(right).cmp(&semver_key(left))
    });
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

    dylibs.sort_by(|left, right| {
        let semver_key = |p: &PathBuf| {
            p.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("libonnxruntime.so."))
                .map(parse_semver)
                .unwrap_or_default()
        };
        semver_key(right).cmp(&semver_key(left))
    });
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
const _: fn(&Settings) -> Result<KittenBackend, AppError> = KittenBackend::from_settings;
#[cfg(feature = "real-backend")]
const _: LoadBackendFn = KittenBackend::from_model_dir;
#[cfg(feature = "real-backend")]
const _: fn(&str) -> Result<KittenBackend, AppError> = KittenBackend::from_repo;
#[cfg(feature = "real-backend")]
const _: fn() = setup_ort_before_runtime;
#[cfg(feature = "real-backend")]
const _: fn() -> Option<&'static OrtRuntimeMetadata> = cached_ort_metadata;
#[cfg(feature = "real-backend")]
const _: fn() -> OrtRuntimeMetadata = apply_ort_dylib_path;
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
    fn backend_synthesis_path_forces_clean_text_false() {
        let request = InternalSynthesisRequest {
            text: "Hello from the HTTP service".to_string(),
            voice_id: Some("jasper".to_string()),
            model_id: None,
            speed: 1.0,
            output_format: Some("wav".to_string()),
            streaming: false,
        };
        let seen_clean_text = std::sync::Arc::new(Mutex::new(None));
        let seen_clean_text_in_generate = std::sync::Arc::clone(&seen_clean_text);

        let waveform = generate_waveform_with_clean_text(
            &request,
            move |_text, _voice, _speed, clean_text| {
                *seen_clean_text_in_generate.lock().unwrap() = Some(clean_text);
                Ok::<Vec<f32>, std::io::Error>(vec![0.0, 0.25, -0.25])
            },
        )
        .unwrap();

        assert_eq!(waveform, vec![0.0, 0.25, -0.25]);
        assert_eq!(*seen_clean_text.lock().unwrap(), Some(false));
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
    fn apply_ort_dylib_path_respects_existing_env_var() {
        let _guard = env_guard();
        env::set_var(ORT_DYLIB_ENV, "/already/configured/libonnxruntime.so");

        let metadata = apply_ort_dylib_path();

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
    fn apply_ort_dylib_path_sets_local_candidate() {
        let _guard = env_guard();
        let home_dir = temp_model_dir("ort-home-configure");
        let ort_dir = home_dir.join(".local/share/onnxruntime/1.24.2");
        fs::create_dir_all(&ort_dir).unwrap();
        let expected_path = ort_dir.join("libonnxruntime.so.1.24.2");
        fs::write(&expected_path, b"fake-ort").unwrap();
        env::set_var("HOME", &home_dir);

        let metadata = apply_ort_dylib_path();

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
    fn apply_ort_dylib_path_reports_system_default_when_nothing_is_found() {
        let _guard = env_guard();
        let home_dir = temp_model_dir("ort-home-system-default-configure");
        env::set_var("HOME", &home_dir);

        let metadata = apply_ort_dylib_path();

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
        // Ensure ORT_DYLIB_PATH is resolved before the backend initialises
        // ONNX Runtime (mimics the pre-runtime setup done in `main`).
        let _ = apply_ort_dylib_path();

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

    #[test]
    fn parse_semver_handles_version_strings_correctly() {
        assert_eq!(parse_semver("1.24.2"), (1, 24, 2));
        assert_eq!(parse_semver("1.9.0"), (1, 9, 0));
        assert_eq!(parse_semver("1.10.0"), (1, 10, 0));
        assert_eq!(parse_semver("2.0.0"), (2, 0, 0));
        assert_eq!(parse_semver("not-a-version"), (0, 0, 0));
        assert_eq!(parse_semver(""), (0, 0, 0));
        // Key correctness check: 1.10.0 > 1.9.0 numerically.
        assert!(parse_semver("1.10.0") > parse_semver("1.9.0"));
        assert!(parse_semver("1.24.2") > parse_semver("1.10.0"));
    }

    #[test]
    fn discover_default_ort_dylib_path_selects_newest_version_numerically() {
        let _guard = env_guard();
        let home_dir = temp_model_dir("ort-home-semver");
        // Create three version directories: 1.9.0 < 1.10.0 < 1.24.2.
        // Lexicographic ordering would put "1.9.0" first (wrong); numeric
        // ordering must select "1.24.2".
        for ver in &["1.9.0", "1.10.0", "1.24.2"] {
            let dir = home_dir.join(format!(".local/share/onnxruntime/{ver}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(format!("libonnxruntime.so.{ver}")), b"fake-ort").unwrap();
        }
        env::set_var("HOME", &home_dir);

        let discovered = discover_default_ort_dylib_path();

        assert_eq!(
            discovered.as_deref(),
            Some(
                home_dir
                    .join(".local/share/onnxruntime/1.24.2/libonnxruntime.so.1.24.2")
                    .as_path()
            )
        );

        fs::remove_dir_all(home_dir).unwrap();
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
