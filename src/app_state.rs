use crate::config::Settings;

#[derive(Clone, Debug)]
pub struct EngineMetadata {
    pub engine: String,
    pub engine_version: Option<String>,
    pub model_loaded: bool,
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
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub engine_metadata: EngineMetadata,
}

impl AppState {
    pub fn new(settings: Settings, engine_metadata: EngineMetadata) -> Self {
        Self {
            settings,
            engine_metadata,
        }
    }
}
