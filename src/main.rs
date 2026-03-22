use kittentts_server_rs::{build_router, init_logging, load_settings, AppState, EngineMetadata};
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), kittentts_server_rs::AppError> {
    let settings = load_settings(None)?;
    init_logging(&settings)?;

    let state = AppState::new(
        settings.clone(),
        EngineMetadata::new("kitten_tts_rs", env!("CARGO_PKG_VERSION"), false),
    );
    let app = build_router(state);

    let listener = TcpListener::bind((settings.host.as_str(), settings.port))
        .await
        .map_err(kittentts_server_rs::AppError::bind_failed)?;
    let local_addr = listener
        .local_addr()
        .map_err(kittentts_server_rs::AppError::bind_failed)?;

    info!(address = %local_addr, "server listening");

    axum::serve(listener, app)
        .await
        .map_err(kittentts_server_rs::AppError::serve_failed)
}
