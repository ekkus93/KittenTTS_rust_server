use crate::error::{AppError, AppErrorCode};
use crate::models::internal::InternalSynthesisRequest;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct VoiceSettings {
    pub speed: Option<f32>,
    pub stability: Option<f32>,
    pub similarity_boost: Option<f32>,
    pub style: Option<f32>,
    pub use_speaker_boost: Option<bool>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl VoiceSettings {
    pub fn unsupported_fields(&self) -> Vec<String> {
        self.extra.keys().cloned().collect()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct TtsRequest {
    pub text: String,
    pub model_id: Option<String>,
    pub voice_settings: Option<VoiceSettings>,
    pub output_format: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl TtsRequest {
    pub fn unsupported_fields(&self) -> Vec<String> {
        self.extra.keys().cloned().collect()
    }

    pub fn validate_compatibility(&self, strict_mode: bool) -> Result<(), AppError> {
        if !strict_mode {
            return Ok(());
        }

        if !self.extra.is_empty() {
            let mut details = BTreeMap::new();
            details.insert(
                "fields".to_string(),
                Value::Array(
                    self.unsupported_fields()
                        .into_iter()
                        .map(Value::String)
                        .collect(),
                ),
            );

            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                AppErrorCode::UnsupportedRequestFields,
                "Unsupported request fields in strict mode",
            )
            .with_details(details));
        }

        if let Some(voice_settings) = &self.voice_settings {
            let unsupported_fields = voice_settings.unsupported_fields();
            if !unsupported_fields.is_empty() {
                let mut details = BTreeMap::new();
                details.insert(
                    "fields".to_string(),
                    Value::Array(unsupported_fields.into_iter().map(Value::String).collect()),
                );

                return Err(AppError::new(
                    StatusCode::BAD_REQUEST,
                    AppErrorCode::UnsupportedVoiceSettings,
                    "Unsupported voice_settings fields in strict mode",
                )
                .with_details(details));
            }
        }

        Ok(())
    }

    pub fn to_internal_request(
        &self,
        requested_voice_id: Option<&str>,
        strict_mode: bool,
        streaming: bool,
    ) -> Result<InternalSynthesisRequest, AppError> {
        self.validate_compatibility(strict_mode)?;

        let text = self.text.trim();
        if text.is_empty() {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                AppErrorCode::MissingText,
                "text is required",
            ));
        }

        let speed = self
            .voice_settings
            .as_ref()
            .and_then(|settings| settings.speed)
            .unwrap_or(1.0);

        Ok(InternalSynthesisRequest {
            text: text.to_string(),
            voice_id: requested_voice_id.map(ToOwned::to_owned),
            model_id: self.model_id.clone(),
            speed,
            output_format: self
                .output_format
                .clone()
                .map(|value| value.trim().to_ascii_lowercase()),
            streaming,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum OpenAiModel {
    #[serde(rename = "gpt-4o-mini-tts")]
    Gpt4oMiniTts,
    #[serde(rename = "tts-1")]
    Tts1,
    #[serde(rename = "tts-1-hd")]
    Tts1Hd,
}

impl OpenAiModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gpt4oMiniTts => "gpt-4o-mini-tts",
            Self::Tts1 => "tts-1",
            Self::Tts1Hd => "tts-1-hd",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum OpenAiResponseFormat {
    #[serde(rename = "wav")]
    Wav,
    #[serde(rename = "pcm")]
    Pcm,
}

impl OpenAiResponseFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Pcm => "pcm",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OpenAiSpeechRequest {
    pub model: OpenAiModel,
    pub voice: String,
    pub input: String,
    pub response_format: Option<OpenAiResponseFormat>,
    pub speed: Option<f32>,
}

impl OpenAiSpeechRequest {
    pub fn to_internal_request(&self) -> Result<InternalSynthesisRequest, AppError> {
        let text = self.input.trim();
        if text.is_empty() {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                AppErrorCode::MissingInput,
                "input is required",
            ));
        }

        Ok(InternalSynthesisRequest {
            text: text.to_string(),
            voice_id: Some(self.voice.clone()),
            model_id: Some(self.model.as_str().to_string()),
            speed: self.speed.unwrap_or(1.0),
            output_format: Some(
                self.response_format
                    .unwrap_or(OpenAiResponseFormat::Wav)
                    .as_str()
                    .to_string(),
            ),
            streaming: false,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub engine: String,
    pub engine_version: Option<String>,
    pub model_loaded: bool,
    pub onnx_runtime_source: Option<String>,
    pub onnx_runtime_path: Option<String>,
    pub default_voice_id: String,
    pub output_format: String,
    pub sample_rate: u32,
    pub channel_layout: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VoiceDescriptor {
    pub voice_id: String,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub preview_url: Option<String>,
    pub available_for_tiers: Vec<String>,
    pub labels: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VoiceListResponse {
    pub voices: Vec<VoiceDescriptor>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_tts_request_parses_and_normalizes_to_internal_request() {
        let payload = json!({
            "text": "  hello world  ",
            "model_id": "kitten-local",
            "voice_settings": {
                "speed": 1.25,
                "stability": 0.5
            },
            "output_format": "WAV"
        });

        let request: TtsRequest = serde_json::from_value(payload).unwrap();
        let internal = request
            .to_internal_request(Some("jasper"), false, false)
            .unwrap();

        assert_eq!(internal.text, "hello world");
        assert_eq!(internal.voice_id.as_deref(), Some("jasper"));
        assert_eq!(internal.model_id.as_deref(), Some("kitten-local"));
        assert_eq!(internal.speed, 1.25);
        assert_eq!(internal.output_format.as_deref(), Some("wav"));
        assert!(!internal.streaming);
    }

    #[test]
    fn empty_text_is_rejected() {
        let request = TtsRequest {
            text: "   ".to_string(),
            ..TtsRequest::default()
        };

        let error = request.to_internal_request(None, false, false).unwrap_err();

        assert_eq!(error.code, AppErrorCode::MissingText);
        assert_eq!(error.message, "text is required");
    }

    #[test]
    fn openai_request_validation_accepts_allowed_values() {
        let payload = json!({
            "model": "tts-1",
            "voice": "bella",
            "input": "Hello there",
            "response_format": "pcm",
            "speed": 0.9
        });

        let request: OpenAiSpeechRequest = serde_json::from_value(payload).unwrap();
        let internal = request.to_internal_request().unwrap();

        assert_eq!(internal.model_id.as_deref(), Some("tts-1"));
        assert_eq!(internal.voice_id.as_deref(), Some("bella"));
        assert_eq!(internal.output_format.as_deref(), Some("pcm"));
        assert_eq!(internal.speed, 0.9);
    }

    #[test]
    fn openai_request_rejects_invalid_model() {
        let payload = json!({
            "model": "tts-9",
            "voice": "bella",
            "input": "Hello there"
        });

        let error = serde_json::from_value::<OpenAiSpeechRequest>(payload).unwrap_err();

        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn strict_mode_rejects_unsupported_top_level_fields() {
        let payload = json!({
            "text": "hello",
            "unknown_field": true
        });

        let request: TtsRequest = serde_json::from_value(payload).unwrap();
        let error = request.to_internal_request(None, true, false).unwrap_err();

        assert_eq!(error.code, AppErrorCode::UnsupportedRequestFields);
        assert_eq!(error.message, "Unsupported request fields in strict mode");
        assert!(error.details.contains_key("fields"));
    }

    #[test]
    fn strict_mode_rejects_unsupported_voice_settings_fields() {
        let payload = json!({
            "text": "hello",
            "voice_settings": {
                "speed": 1.1,
                "experimental": true
            }
        });

        let request: TtsRequest = serde_json::from_value(payload).unwrap();
        let error = request.to_internal_request(None, true, false).unwrap_err();

        assert_eq!(error.code, AppErrorCode::UnsupportedVoiceSettings);
        assert_eq!(
            error.message,
            "Unsupported voice_settings fields in strict mode"
        );
        assert!(error.details.contains_key("fields"));
    }
}
