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
            _ => 1,
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
