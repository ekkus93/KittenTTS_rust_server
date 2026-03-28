use crate::config::Settings;
use crate::error::AppError;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};
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

#[cfg(test)]
mod tests {
    use super::*;

    static LOG_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn log_filter_directive_matches_python_level_mapping() {
        assert_eq!(log_filter_directive("CRITICAL"), "error");
        assert_eq!(log_filter_directive("ERROR"), "error");
        assert_eq!(log_filter_directive("WARNING"), "warn");
        assert_eq!(log_filter_directive("INFO"), "info");
        assert_eq!(log_filter_directive("DEBUG"), "debug");
        assert_eq!(log_filter_directive("UNKNOWN"), "info");
    }

    #[test]
    fn init_logging_fails_when_called_twice() {
        let _guard = LOG_TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let settings = Settings::default();

        init_logging(&settings).expect("first logger initialization should succeed");
        let error = init_logging(&settings).expect_err("second logger initialization should fail");

        assert_eq!(error.code, crate::AppErrorCode::Internal);
        assert!(error.message.contains("failed to initialize logging"));
    }
}
