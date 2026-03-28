use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const ENV_PREFIX: &str = "KITTENTTS_SERVER_";
pub const DEFAULT_CONFIG_PATH: &str = "config/settings.json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub auth_enabled: bool,
    pub local_api_key: Option<String>,
    pub model_dir: Option<PathBuf>,
    pub default_voice_id: String,
    pub default_model_id: String,
    pub voice_map: BTreeMap<String, String>,
    pub output_format: String,
    pub sample_rate: u32,
    pub channel_layout: String,
    pub log_level: String,
    pub strict_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8008,
            auth_enabled: false,
            local_api_key: None,
            model_dir: None,
            default_voice_id: "jasper".to_string(),
            default_model_id: "kitten-local".to_string(),
            voice_map: BTreeMap::new(),
            output_format: "wav".to_string(),
            sample_rate: 24_000,
            channel_layout: "mono".to_string(),
            log_level: "INFO".to_string(),
            strict_mode: false,
        }
    }
}

impl Settings {
    pub fn output_channels(&self) -> u16 {
        match self.channel_layout.as_str() {
            "mono" => 1,
            "stereo" => 2,
            _ => unreachable!("channel_layout validated at config load time"),
        }
    }

    pub fn validate(mut self) -> Result<Self, AppError> {
        self.output_format = self.output_format.to_ascii_lowercase();
        self.channel_layout = self.channel_layout.to_ascii_lowercase();
        self.log_level = self.log_level.to_ascii_uppercase();

        if self.auth_enabled && self.local_api_key.as_deref().unwrap_or_default().is_empty() {
            return Err(AppError::invalid_config(
                "local_api_key must be set when auth_enabled is true",
            ));
        }

        if self
            .model_dir
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(AppError::invalid_config("model_dir must not be empty"));
        }

        if self.port == 0 {
            return Err(AppError::invalid_config("port must be between 1 and 65535"));
        }

        if self.sample_rate == 0 {
            return Err(AppError::invalid_config(
                "sample_rate must be a positive integer",
            ));
        }

        if self.output_format != "wav" {
            return Err(AppError::invalid_config(
                "only wav output is supported in v1",
            ));
        }

        if !matches!(self.channel_layout.as_str(), "mono" | "stereo") {
            return Err(AppError::invalid_config(
                "channel_layout must be either 'mono' or 'stereo'",
            ));
        }

        if !matches!(
            self.log_level.as_str(),
            "CRITICAL" | "ERROR" | "WARNING" | "INFO" | "DEBUG"
        ) {
            return Err(AppError::invalid_config(
                "log_level must be one of CRITICAL, ERROR, WARNING, INFO, DEBUG",
            ));
        }

        Ok(self)
    }
}

/// Load and validate server settings from defaults, environment variables, and
/// an optional JSON config file.
///
/// Merge order (last write wins):
/// 1. Built-in defaults (`Settings::default()`)
/// 2. `KITTENTTS_SERVER_*` environment variables
/// 3. JSON config file (from `config_path`, `KITTENTTS_SERVER_CONFIG_FILE`, or
///    the default path `config/settings.json` when it exists)
///
/// **Note:** the config file takes final precedence over environment variables.
/// This matches the Python server's documented behavior and is intentional.
/// Operators who want env-only configuration should omit the config file rather
/// than rely on environment variables to override it.
pub fn load_settings(config_path: Option<PathBuf>) -> Result<Settings, AppError> {
    let config_path = selected_config_path(config_path);
    let mut merged = default_config_values()?;

    for (key, value) in environment_overrides()? {
        merged.insert(key, value);
    }

    if let Some(path) = config_path {
        for (key, value) in load_json_config(&path)? {
            merged.insert(key, value);
        }
    }

    serde_json::from_value::<Settings>(Value::Object(merged))
        .map_err(|err| AppError::invalid_config(format!("invalid configuration: {err}")))?
        .validate()
}

fn default_config_values() -> Result<Map<String, Value>, AppError> {
    let value = serde_json::to_value(Settings::default()).map_err(|err| {
        AppError::internal(format!("failed to serialize default settings: {err}"))
    })?;

    match value {
        Value::Object(map) => Ok(map),
        _ => Err(AppError::internal(
            "default settings must serialize to a JSON object",
        )),
    }
}

fn selected_config_path(config_path: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = config_path {
        return Some(path);
    }

    if let Some(path) = env::var_os(format!("{ENV_PREFIX}CONFIG_FILE")) {
        return Some(PathBuf::from(path));
    }

    let default_path = PathBuf::from(DEFAULT_CONFIG_PATH);
    if default_path.exists() {
        Some(default_path)
    } else {
        None
    }
}

fn load_json_config(path: &Path) -> Result<Map<String, Value>, AppError> {
    let contents = fs::read_to_string(path).map_err(|err| {
        AppError::invalid_config(format!("failed to read config {}: {err}", path.display()))
    })?;
    let value: Value = serde_json::from_str(&contents).map_err(|err| {
        AppError::invalid_config(format!("invalid JSON in {}: {err}", path.display()))
    })?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(AppError::invalid_config(format!(
            "config {} must contain a JSON object",
            path.display()
        ))),
    }
}

fn environment_overrides() -> Result<Map<String, Value>, AppError> {
    let mut overrides = Map::new();

    insert_string(&mut overrides, "HOST", "host");
    insert_string(&mut overrides, "LOCAL_API_KEY", "local_api_key");
    insert_string(&mut overrides, "MODEL_DIR", "model_dir");
    insert_string(&mut overrides, "DEFAULT_VOICE_ID", "default_voice_id");
    insert_string(&mut overrides, "DEFAULT_MODEL_ID", "default_model_id");
    insert_string(&mut overrides, "OUTPUT_FORMAT", "output_format");
    insert_string(&mut overrides, "CHANNEL_LAYOUT", "channel_layout");
    insert_string(&mut overrides, "LOG_LEVEL", "log_level");
    insert_bool(&mut overrides, "AUTH_ENABLED", "auth_enabled")?;
    insert_bool(&mut overrides, "STRICT_MODE", "strict_mode")?;
    insert_u64(&mut overrides, "PORT", "port")?;
    insert_u64(&mut overrides, "SAMPLE_RATE", "sample_rate")?;

    if let Some(voice_map) = env_value("VOICE_MAP") {
        let value = parse_json_mapping(&voice_map, &format!("{ENV_PREFIX}VOICE_MAP"))?;
        overrides.insert(
            "voice_map".to_string(),
            serde_json::to_value(value).map_err(|err| {
                AppError::internal(format!("failed to serialize voice_map override: {err}"))
            })?,
        );
    }

    Ok(overrides)
}

fn insert_string(overrides: &mut Map<String, Value>, env_suffix: &str, field: &str) {
    if let Some(value) = env_value(env_suffix) {
        overrides.insert(field.to_string(), Value::String(value));
    }
}

fn insert_bool(
    overrides: &mut Map<String, Value>,
    env_suffix: &str,
    field: &str,
) -> Result<(), AppError> {
    if let Some(value) = env_value(env_suffix) {
        let parsed = parse_bool(&value)?;
        overrides.insert(field.to_string(), Value::Bool(parsed));
    }
    Ok(())
}

fn insert_u64(
    overrides: &mut Map<String, Value>,
    env_suffix: &str,
    field: &str,
) -> Result<(), AppError> {
    if let Some(value) = env_value(env_suffix) {
        let parsed = value.parse::<u64>().map_err(|err| {
            AppError::invalid_config(format!(
                "{ENV_PREFIX}{env_suffix} must be an integer: {err}"
            ))
        })?;
        overrides.insert(field.to_string(), Value::Number(parsed.into()));
    }
    Ok(())
}

fn env_value(env_suffix: &str) -> Option<String> {
    env::var(format!("{ENV_PREFIX}{env_suffix}")).ok()
}

fn parse_bool(value: &str) -> Result<bool, AppError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(AppError::invalid_config(format!(
            "invalid boolean value: {value:?}"
        ))),
    }
}

fn parse_json_mapping(value: &str, env_name: &str) -> Result<BTreeMap<String, String>, AppError> {
    serde_json::from_str::<BTreeMap<String, String>>(value).map_err(|err| {
        AppError::invalid_config(format!(
            "{env_name} must be a JSON object with string values: {err}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    static UNIQUE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct EnvResetGuard {
        saved_config_file: Option<String>,
        saved_cwd: PathBuf,
        temp_dir: Option<PathBuf>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for EnvResetGuard {
        fn drop(&mut self) {
            match &self.saved_config_file {
                Some(value) => env::set_var(format!("{ENV_PREFIX}CONFIG_FILE"), value),
                None => env::remove_var(format!("{ENV_PREFIX}CONFIG_FILE")),
            }
            env::set_current_dir(&self.saved_cwd).unwrap();
            if let Some(temp_dir) = &self.temp_dir {
                let _ = fs::remove_dir_all(temp_dir);
            }
        }
    }

    fn env_guard() -> EnvResetGuard {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        EnvResetGuard {
            saved_config_file: env::var(format!("{ENV_PREFIX}CONFIG_FILE")).ok(),
            saved_cwd: env::current_dir().unwrap(),
            temp_dir: None,
            _lock: lock,
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!(
            "kittentts-server-rs-config-unit-{label}-{}-{unique}",
            process::id()
        ))
    }

    #[test]
    fn output_channels_matches_validated_layouts() {
        let mono = Settings {
            channel_layout: "mono".to_string(),
            ..Settings::default()
        };
        let stereo = Settings {
            channel_layout: "stereo".to_string(),
            ..Settings::default()
        };

        assert_eq!(mono.output_channels(), 1);
        assert_eq!(stereo.output_channels(), 2);
    }

    #[test]
    fn selected_config_path_prefers_explicit_path_over_env() {
        let _guard = env_guard();
        env::set_var(
            format!("{ENV_PREFIX}CONFIG_FILE"),
            "/tmp/from-env-settings.json",
        );

        let selected = selected_config_path(Some(PathBuf::from("/tmp/from-arg-settings.json")));

        assert_eq!(selected, Some(PathBuf::from("/tmp/from-arg-settings.json")));
    }

    #[test]
    fn selected_config_path_uses_env_when_no_explicit_path_is_provided() {
        let _guard = env_guard();
        env::set_var(
            format!("{ENV_PREFIX}CONFIG_FILE"),
            "/tmp/from-env-settings.json",
        );

        let selected = selected_config_path(None);

        assert_eq!(selected, Some(PathBuf::from("/tmp/from-env-settings.json")));
    }

    #[test]
    fn selected_config_path_uses_default_path_when_present() {
        let mut guard = env_guard();
        let temp_dir = unique_temp_dir("default-config-path");
        fs::create_dir_all(temp_dir.join("config")).unwrap();
        fs::write(temp_dir.join(DEFAULT_CONFIG_PATH), b"{}").unwrap();
        env::set_current_dir(&temp_dir).unwrap();
        guard.temp_dir = Some(temp_dir.clone());

        let selected = selected_config_path(None);

        assert_eq!(selected, Some(PathBuf::from(DEFAULT_CONFIG_PATH)));
    }

    #[test]
    fn selected_config_path_returns_none_when_no_source_exists() {
        let mut guard = env_guard();
        let temp_dir = unique_temp_dir("no-config-path");
        fs::create_dir_all(&temp_dir).unwrap();
        env::set_current_dir(&temp_dir).unwrap();
        guard.temp_dir = Some(temp_dir);

        let selected = selected_config_path(None);

        assert_eq!(selected, None);
    }

    #[test]
    fn parse_bool_accepts_all_supported_aliases() {
        for value in ["1", "true", "yes", "on", "TRUE", " Yes "] {
            assert!(parse_bool(value).unwrap());
        }

        for value in ["0", "false", "no", "off", "FALSE", " Off "] {
            assert!(!parse_bool(value).unwrap());
        }
    }
}
