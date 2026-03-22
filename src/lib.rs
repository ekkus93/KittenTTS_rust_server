pub mod app_state;
pub mod config;
pub mod error;
pub mod logging;
pub mod models;

pub(crate) mod backend;
pub(crate) mod middleware;
pub(crate) mod routes;
pub(crate) mod services;

pub use app_state::{AppState, EngineMetadata};
pub use config::{load_settings, Settings};
pub use error::{AppError, AppErrorCode, LocalErrorEnvelope, OpenAiErrorEnvelope};
pub use logging::init_logging;
pub use routes::build_router;

/// Resolves and records the ONNX Runtime shared library path.  Call this
/// **before** building the tokio runtime so that `env::set_var` runs in a
/// single-threaded context.
pub fn setup_ort_before_runtime() {
    #[cfg(feature = "real-backend")]
    backend::kitten::setup_ort_before_runtime();
}
