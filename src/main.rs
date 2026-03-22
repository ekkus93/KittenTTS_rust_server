use kittentts_server_rs::{
    app_state::initialize_app_state, build_router, init_logging, load_settings,
    setup_ort_before_runtime,
};
use tokio::net::TcpListener;
use tokio::task;
use tracing::info;

fn main() -> Result<(), kittentts_server_rs::AppError> {
    let settings = load_settings(None)?;

    // Resolve the ONNX Runtime shared library path before the tokio runtime
    // starts so that env::set_var runs in a single-threaded context.
    setup_ort_before_runtime();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| {
            kittentts_server_rs::AppError::internal(format!("failed to build tokio runtime: {err}"))
        })?
        .block_on(async move {
            init_logging(&settings)?;

            let state = task::spawn_blocking({
                let settings = settings.clone();
                move || initialize_app_state(settings)
            })
            .await
            .map_err(|err| {
                kittentts_server_rs::AppError::internal(format!(
                    "backend initialization task failed: {err}"
                ))
            })??;

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
        })
}
