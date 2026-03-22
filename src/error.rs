use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    InvalidConfig,
    Validation,
    MissingText,
    MissingInput,
    UnsupportedRequestFields,
    UnsupportedVoiceSettings,
    BindFailed,
    ServeFailed,
    Internal,
}

impl AppErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidConfig => "invalid_config",
            Self::Validation => "validation_error",
            Self::MissingText => "missing_text",
            Self::MissingInput => "missing_input",
            Self::UnsupportedRequestFields => "unsupported_request_fields",
            Self::UnsupportedVoiceSettings => "unsupported_voice_settings",
            Self::BindFailed => "bind_failed",
            Self::ServeFailed => "serve_failed",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct AppError {
    pub status: StatusCode,
    pub code: AppErrorCode,
    pub message: String,
    pub details: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct LocalErrorEnvelope {
    pub error: LocalErrorBody,
}

#[derive(Debug, Serialize)]
pub struct LocalErrorBody {
    pub code: String,
    pub message: String,
    pub details: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpenAiErrorEnvelope {
    pub error: OpenAiErrorBody,
}

#[derive(Debug, Serialize)]
pub struct OpenAiErrorBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
}

impl AppError {
    pub fn new(status: StatusCode, code: AppErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: BTreeMap::new(),
        }
    }

    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            AppErrorCode::InvalidConfig,
            message,
        )
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, AppErrorCode::Validation, message)
    }

    pub fn bind_failed(error: impl std::fmt::Display) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            AppErrorCode::BindFailed,
            format!("failed to bind server: {error}"),
        )
    }

    pub fn serve_failed(error: impl std::fmt::Display) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            AppErrorCode::ServeFailed,
            format!("server exited with error: {error}"),
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            AppErrorCode::Internal,
            message,
        )
    }

    pub fn with_details(mut self, details: BTreeMap<String, Value>) -> Self {
        self.details = details;
        self
    }

    pub fn into_local_envelope(&self) -> LocalErrorEnvelope {
        LocalErrorEnvelope {
            error: LocalErrorBody {
                code: self.code.as_str().to_string(),
                message: self.message.clone(),
                details: self.details.clone(),
                request_id: None,
            },
        }
    }

    pub fn into_openai_envelope(&self) -> OpenAiErrorEnvelope {
        OpenAiErrorEnvelope {
            error: OpenAiErrorBody {
                code: Some(self.code.as_str().to_string()),
                message: self.message.clone(),
                error_type: self.code.as_str().to_string(),
            },
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(self.into_local_envelope())).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_error_envelope_includes_details() {
        let mut details = BTreeMap::new();
        details.insert(
            "fields".to_string(),
            Value::Array(vec![Value::String("voice_settings.extra".to_string())]),
        );

        let envelope = AppError::validation("Unsupported request fields")
            .with_details(details)
            .into_local_envelope();

        assert_eq!(envelope.error.code, "validation_error");
        assert_eq!(envelope.error.message, "Unsupported request fields");
        assert!(envelope.error.details.contains_key("fields"));
        assert!(envelope.error.request_id.is_none());
    }

    #[test]
    fn openai_error_envelope_matches_expected_shape() {
        let envelope = AppError::new(
            StatusCode::BAD_REQUEST,
            AppErrorCode::MissingInput,
            "input is required",
        )
        .into_openai_envelope();

        assert_eq!(envelope.error.code.as_deref(), Some("missing_input"));
        assert_eq!(envelope.error.message, "input is required");
        assert_eq!(envelope.error.error_type, "missing_input");
    }
}
