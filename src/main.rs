use kittentts_server_rs::{
    app_state::initialize_app_state, build_router, init_logging, load_settings,
    setup_ort_before_runtime, AppError, AppState, Settings,
};
#[cfg(test)]
use kittentts_server_rs::{AppErrorCode, EngineMetadata};
use std::io;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::task;
use tracing::info;

fn main() -> Result<(), AppError> {
    let settings = load_settings(None)?;

    run(settings)
}

fn run(settings: Settings) -> Result<(), AppError> {
    // Resolve the ONNX Runtime shared library path before the tokio runtime
    // starts so that env::set_var runs in a single-threaded context.
    setup_ort_before_runtime();

    let runtime = build_runtime()?;

    runtime.block_on(async move {
        init_logging(&settings)?;

        let state = initialize_state(settings.clone()).await?;

        let app = build_router(state);

        let listener = bind_listener(settings.host.as_str(), settings.port).await?;
        let local_addr = listener.local_addr().map_err(AppError::bind_failed)?;

        info!(address = %local_addr, "server listening");

        axum::serve(listener, app)
            .await
            .map_err(AppError::serve_failed)
    })
}

fn build_runtime() -> Result<Runtime, AppError> {
    build_runtime_with(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
    })
}

fn build_runtime_with<F>(build: F) -> Result<Runtime, AppError>
where
    F: FnOnce() -> io::Result<Runtime>,
{
    build().map_err(|err| AppError::internal(format!("failed to build tokio runtime: {err}")))
}

async fn initialize_state(settings: Settings) -> Result<AppState, AppError> {
    initialize_state_with(settings, initialize_app_state).await
}

async fn initialize_state_with<F>(settings: Settings, initialize: F) -> Result<AppState, AppError>
where
    F: FnOnce(Settings) -> Result<AppState, AppError> + Send + 'static,
{
    task::spawn_blocking(move || initialize(settings))
        .await
        .map_err(|err| AppError::internal(format!("backend initialization task failed: {err}")))?
}

async fn bind_listener(host: &str, port: u16) -> Result<TcpListener, AppError> {
    TcpListener::bind((host, port))
        .await
        .map_err(AppError::bind_failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    fn test_state(settings: Settings) -> AppState {
        AppState::new(settings, EngineMetadata::new("kitten_tts_rs", "test", true))
    }

    #[test]
    fn build_runtime_with_maps_runtime_builder_errors() {
        let error = build_runtime_with(|| Err(io::Error::other("runtime boom"))).unwrap_err();

        assert_eq!(error.code, AppErrorCode::Internal);
        assert!(error.message.contains("failed to build tokio runtime"));
        assert!(error.message.contains("runtime boom"));
    }

    #[tokio::test]
    async fn initialize_state_with_propagates_app_errors() {
        let error = match initialize_state_with(Settings::default(), |_| {
            Err(AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                AppErrorCode::BackendUnavailable,
                "backend offline",
            ))
        })
        .await
        {
            Ok(_) => panic!("expected backend initialization error"),
            Err(error) => error,
        };

        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.code, AppErrorCode::BackendUnavailable);
        assert_eq!(error.message, "backend offline");
    }

    #[tokio::test]
    async fn initialize_state_with_maps_join_failures() {
        let error = match initialize_state_with(Settings::default(), |_| panic!("boom")).await {
            Ok(_) => panic!("expected backend initialization join failure"),
            Err(error) => error,
        };

        assert_eq!(error.code, AppErrorCode::Internal);
        assert!(error.message.contains("backend initialization task failed"));
    }

    #[tokio::test]
    async fn bind_listener_reports_bind_failed_when_port_is_in_use() {
        let existing = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = existing.local_addr().unwrap().port();

        let error = bind_listener("127.0.0.1", port).await.unwrap_err();

        assert_eq!(error.code, AppErrorCode::BindFailed);
        assert!(error.message.contains("failed to bind server"));
    }

    #[tokio::test]
    async fn initialize_state_with_returns_initialized_state() {
        let settings = Settings::default();
        let state = initialize_state_with(settings.clone(), |settings| Ok(test_state(settings)))
            .await
            .unwrap();

        assert_eq!(state.settings, settings);
        assert_eq!(state.engine_metadata.engine, "kitten_tts_rs");
        assert!(state.engine_metadata.model_loaded);
    }
}
