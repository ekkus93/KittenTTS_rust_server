use crate::config::Settings;
use crate::error::AppError;
use tracing_subscriber::EnvFilter;

pub fn init_logging(settings: &Settings) -> Result<(), AppError> {
    let env_filter = EnvFilter::try_new(log_filter_directive(&settings.log_level))
        .map_err(|err| AppError::invalid_config(format!("invalid log level filter: {err}")))?;

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .without_time()
        .try_init()
        .map_err(|err| AppError::internal(format!("failed to initialize logging: {err}")))
}

fn log_filter_directive(log_level: &str) -> &'static str {
    match log_level {
        "CRITICAL" => "error",
        "ERROR" => "error",
        "WARNING" => "warn",
        "INFO" => "info",
        "DEBUG" => "debug",
        _ => "info",
    }
}
