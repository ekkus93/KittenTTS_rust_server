#[cfg(feature = "real-backend")]
use kittentts_server_rs::app_state::initialize_app_state;
use kittentts_server_rs::{load_settings, AppErrorCode, Settings};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::{env, process};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static UNIQUE_COUNTER: AtomicUsize = AtomicUsize::new(0);

const TEST_ENV_KEYS: &[&str] = &[
    "KITTENTTS_SERVER_CONFIG_FILE",
    "KITTENTTS_SERVER_HOST",
    "KITTENTTS_SERVER_PORT",
    "KITTENTTS_SERVER_AUTH_ENABLED",
    "KITTENTTS_SERVER_LOCAL_API_KEY",
    "KITTENTTS_SERVER_MODEL_DIR",
    "KITTENTTS_SERVER_DEFAULT_VOICE_ID",
    "KITTENTTS_SERVER_DEFAULT_MODEL_ID",
    "KITTENTTS_SERVER_VOICE_MAP",
    "KITTENTTS_SERVER_OUTPUT_FORMAT",
    "KITTENTTS_SERVER_SAMPLE_RATE",
    "KITTENTTS_SERVER_CHANNEL_LAYOUT",
    "KITTENTTS_SERVER_LOG_LEVEL",
    "KITTENTTS_SERVER_STRICT_MODE",
];

#[test]
fn defaults_match_expected_runtime_settings() {
    let _guard = env_guard();

    let settings = load_settings(None).unwrap();

    assert_eq!(settings, Settings::default());
}

#[test]
fn loads_settings_from_json_config_file() {
    let _guard = env_guard();
    let config_path = write_temp_config(
        r#"{
            "host": "0.0.0.0",
            "port": 9001,
            "auth_enabled": true,
            "local_api_key": "secret",
            "model_dir": "/srv/kitten/model",
            "default_voice_id": "luna",
            "default_model_id": "kitten-custom",
            "voice_map": {"narrator": "jasper"},
            "output_format": "wav",
            "sample_rate": 22050,
            "channel_layout": "stereo",
            "log_level": "warning",
            "strict_mode": true
        }"#,
    );

    let settings = load_settings(Some(config_path)).unwrap();

    assert_eq!(settings.host, "0.0.0.0");
    assert_eq!(settings.port, 9001);
    assert!(settings.auth_enabled);
    assert_eq!(settings.local_api_key.as_deref(), Some("secret"));
    assert_eq!(settings.model_dir, Some(PathBuf::from("/srv/kitten/model")));
    assert_eq!(settings.default_voice_id, "luna");
    assert_eq!(settings.default_model_id, "kitten-custom");
    assert_eq!(
        settings.voice_map.get("narrator").map(String::as_str),
        Some("jasper")
    );
    assert_eq!(settings.output_format, "wav");
    assert_eq!(settings.sample_rate, 22050);
    assert_eq!(settings.channel_layout, "stereo");
    assert_eq!(settings.log_level, "WARNING");
    assert!(settings.strict_mode);
}

#[test]
fn config_file_overrides_environment_to_match_python_precedence() {
    let _guard = env_guard();
    env::set_var("KITTENTTS_SERVER_HOST", "127.0.0.9");
    env::set_var("KITTENTTS_SERVER_PORT", "9999");

    let config_path = write_temp_config(
        r#"{
            "host": "0.0.0.0",
            "port": 8008,
            "output_format": "wav"
        }"#,
    );

    let settings = load_settings(Some(config_path)).unwrap();

    assert_eq!(settings.host, "0.0.0.0");
    assert_eq!(settings.port, 8008);
}

#[test]
fn loads_environment_overrides_without_config_file() {
    let _guard = env_guard();
    env::set_var("KITTENTTS_SERVER_HOST", "0.0.0.0");
    env::set_var("KITTENTTS_SERVER_PORT", "9100");
    env::set_var("KITTENTTS_SERVER_AUTH_ENABLED", "true");
    env::set_var("KITTENTTS_SERVER_LOCAL_API_KEY", "env-secret");
    env::set_var("KITTENTTS_SERVER_MODEL_DIR", "/opt/kitten/model");
    env::set_var("KITTENTTS_SERVER_DEFAULT_VOICE_ID", "bruno");
    env::set_var("KITTENTTS_SERVER_DEFAULT_MODEL_ID", "env-model");
    env::set_var("KITTENTTS_SERVER_VOICE_MAP", r#"{"alias":"bella"}"#);
    env::set_var("KITTENTTS_SERVER_OUTPUT_FORMAT", "WAV");
    env::set_var("KITTENTTS_SERVER_SAMPLE_RATE", "16000");
    env::set_var("KITTENTTS_SERVER_CHANNEL_LAYOUT", "STEREO");
    env::set_var("KITTENTTS_SERVER_LOG_LEVEL", "debug");
    env::set_var("KITTENTTS_SERVER_STRICT_MODE", "yes");

    let settings = load_settings(None).unwrap();

    assert_eq!(settings.host, "0.0.0.0");
    assert_eq!(settings.port, 9100);
    assert!(settings.auth_enabled);
    assert_eq!(settings.local_api_key.as_deref(), Some("env-secret"));
    assert_eq!(settings.model_dir, Some(PathBuf::from("/opt/kitten/model")));
    assert_eq!(settings.default_voice_id, "bruno");
    assert_eq!(settings.default_model_id, "env-model");
    assert_eq!(
        settings.voice_map.get("alias").map(String::as_str),
        Some("bella")
    );
    assert_eq!(settings.output_format, "wav");
    assert_eq!(settings.sample_rate, 16000);
    assert_eq!(settings.channel_layout, "stereo");
    assert_eq!(settings.log_level, "DEBUG");
    assert!(settings.strict_mode);
}

#[test]
fn rejects_invalid_boolean_env_override() {
    let _guard = env_guard();
    env::set_var("KITTENTTS_SERVER_AUTH_ENABLED", "maybe");

    let error = load_settings(None).unwrap_err();

    assert!(error.message.contains("invalid boolean value"));
}

#[test]
fn rejects_invalid_integer_env_override() {
    let _guard = env_guard();
    env::set_var("KITTENTTS_SERVER_PORT", "nine-thousand");

    let error = load_settings(None).unwrap_err();

    assert!(error
        .message
        .contains("KITTENTTS_SERVER_PORT must be an integer"));
}

#[test]
fn rejects_invalid_json_map_env_override() {
    let _guard = env_guard();
    env::set_var("KITTENTTS_SERVER_VOICE_MAP", r#"["bella"]"#);

    let error = load_settings(None).unwrap_err();

    assert!(error
        .message
        .contains("KITTENTTS_SERVER_VOICE_MAP must be a JSON object with string values"));
}

#[test]
fn rejects_invalid_channel_layout() {
    let _guard = env_guard();
    let config_path = write_temp_config(
        r#"{
            "channel_layout": "surround",
            "output_format": "wav"
        }"#,
    );

    let error = load_settings(Some(config_path)).unwrap_err();

    assert!(error
        .message
        .contains("channel_layout must be either 'mono' or 'stereo'"));
}

#[test]
fn rejects_invalid_log_level() {
    let _guard = env_guard();
    let config_path = write_temp_config(
        r#"{
            "log_level": "VERBOSE",
            "output_format": "wav"
        }"#,
    );

    let error = load_settings(Some(config_path)).unwrap_err();

    assert!(error
        .message
        .contains("log_level must be one of CRITICAL, ERROR, WARNING, INFO, DEBUG"));
}

#[test]
fn rejects_empty_model_dir() {
    let _guard = env_guard();
    env::set_var("KITTENTTS_SERVER_MODEL_DIR", "");

    let error = load_settings(None).unwrap_err();

    assert!(error.message.contains("model_dir must not be empty"));
}

#[test]
fn startup_fails_with_invalid_config() {
    let _guard = env_guard();
    let config_path = write_temp_config(
        r#"{
            "log_level": "VERBOSE",
            "output_format": "wav"
        }"#,
    );

    let error = load_settings(Some(config_path)).unwrap_err();

    assert_eq!(error.code, AppErrorCode::InvalidConfig);
    assert!(error
        .message
        .contains("log_level must be one of CRITICAL, ERROR, WARNING, INFO, DEBUG"));
}

#[cfg(feature = "real-backend")]
#[test]
fn startup_fails_with_missing_model_file() {
    let _guard = env_guard();
    let model_dir = write_temp_model_dir(Some(b"placeholder-voices"), None);
    let config_path = write_temp_config(&format!(
        r#"{{
            "model_dir": "{}",
            "output_format": "wav"
        }}"#,
        model_dir.display()
    ));

    let settings = load_settings(Some(config_path)).unwrap();
    let error = match initialize_app_state(settings) {
        Ok(_) => panic!("expected startup to fail when model.onnx is missing"),
        Err(error) => error,
    };

    assert_eq!(error.code, AppErrorCode::BackendUnavailable);
    assert!(error.message.contains("model.onnx"));

    fs::remove_dir_all(model_dir).unwrap();
}

#[cfg(feature = "real-backend")]
#[test]
fn startup_fails_with_missing_voices_file() {
    let _guard = env_guard();
    let model_dir = write_temp_model_dir(None, Some(b"placeholder-model"));
    let config_path = write_temp_config(&format!(
        r#"{{
            "model_dir": "{}",
            "output_format": "wav"
        }}"#,
        model_dir.display()
    ));

    let settings = load_settings(Some(config_path)).unwrap();
    let error = match initialize_app_state(settings) {
        Ok(_) => panic!("expected startup to fail when voices.npz is missing"),
        Err(error) => error,
    };

    assert_eq!(error.code, AppErrorCode::BackendUnavailable);
    assert!(error.message.contains("voices.npz"));

    fs::remove_dir_all(model_dir).unwrap();
}

struct EnvResetGuard {
    saved: Vec<(&'static str, Option<String>)>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for EnvResetGuard {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}

fn env_guard() -> EnvResetGuard {
    let lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let saved = TEST_ENV_KEYS
        .iter()
        .map(|key| (*key, env::var(key).ok()))
        .collect();

    for key in TEST_ENV_KEYS {
        env::remove_var(key);
    }

    EnvResetGuard { saved, _lock: lock }
}

fn write_temp_config(contents: &str) -> PathBuf {
    let path = unique_temp_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, contents).unwrap();
    path
}

fn write_temp_model_dir(voices_contents: Option<&[u8]>, model_contents: Option<&[u8]>) -> PathBuf {
    let path = unique_temp_model_dir();
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("config.json"),
        r#"{"model_file":"model.onnx","voices":"voices.npz"}"#,
    )
    .unwrap();
    if let Some(voices_contents) = voices_contents {
        fs::write(path.join("voices.npz"), voices_contents).unwrap();
    }
    if let Some(model_contents) = model_contents {
        fs::write(path.join("model.onnx"), model_contents).unwrap();
    }
    path
}

fn unique_temp_path() -> PathBuf {
    let unique = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "kittentts-server-rs-config-{}-{unique}.json",
        process::id()
    ))
}

fn unique_temp_model_dir() -> PathBuf {
    let unique = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "kittentts-server-rs-model-dir-{}-{unique}",
        process::id()
    ))
}
