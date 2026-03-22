use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub engine: String,
    pub engine_version: Option<String>,
    pub model_loaded: bool,
    pub default_voice_id: String,
    pub output_format: String,
    pub sample_rate: u32,
    pub channel_layout: String,
}
