use crate::config::Settings;
use crate::error::AppError;
use crate::services::synth::{create_synth_runtime, unavailable_runtime, SynthRuntime};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct EngineMetadata {
    pub engine: String,
    pub engine_version: Option<String>,
    pub model_loaded: bool,
    pub onnx_runtime_source: Option<String>,
    pub onnx_runtime_path: Option<PathBuf>,
}

impl EngineMetadata {
    pub fn new(
        engine: impl Into<String>,
        engine_version: impl Into<String>,
        model_loaded: bool,
    ) -> Self {
        Self {
            engine: engine.into(),
            engine_version: Some(engine_version.into()),
            model_loaded,
            onnx_runtime_source: None,
            onnx_runtime_path: None,
        }
    }

    pub(crate) fn from_runtime(runtime: &SynthRuntime) -> Self {
        Self {
            engine: runtime.engine_name().to_string(),
            engine_version: runtime.engine_version().map(str::to_string),
            model_loaded: runtime.model_loaded(),
            onnx_runtime_source: runtime.onnx_runtime_source().map(str::to_string),
            onnx_runtime_path: runtime.onnx_runtime_path().cloned(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub engine_metadata: EngineMetadata,
    pub(crate) synth_runtime: SynthRuntime,
}

impl AppState {
    pub fn new(settings: Settings, engine_metadata: EngineMetadata) -> Self {
        let synth_runtime = unavailable_runtime(&settings);

        Self {
            settings,
            engine_metadata,
            synth_runtime,
        }
    }

    pub(crate) fn from_runtime(settings: Settings, synth_runtime: SynthRuntime) -> Self {
        let engine_metadata = EngineMetadata::from_runtime(&synth_runtime);

        Self {
            settings,
            engine_metadata,
            synth_runtime,
        }
    }
}

pub fn initialize_app_state(settings: Settings) -> Result<AppState, AppError> {
    let synth_runtime = create_synth_runtime(&settings)?;
    Ok(AppState::from_runtime(settings, synth_runtime))
}
