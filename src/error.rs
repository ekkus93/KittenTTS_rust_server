use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    InvalidConfig,
    BindFailed,
    ServeFailed,
    Internal,
}

impl AppErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidConfig => "invalid_config",
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
}

#[derive(Debug, Serialize)]
pub struct LocalErrorEnvelope {
    pub error: LocalErrorBody,
}

#[derive(Debug, Serialize)]
pub struct LocalErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct OpenAiErrorEnvelope {
    pub error: OpenAiErrorBody,
}

#[derive(Debug, Serialize)]
pub struct OpenAiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
}

impl AppError {
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: AppErrorCode::InvalidConfig,
            message: message.into(),
        }
    }

    pub fn bind_failed(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: AppErrorCode::BindFailed,
            message: format!("failed to bind server: {error}"),
        }
    }

    pub fn serve_failed(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: AppErrorCode::ServeFailed,
            message: format!("server exited with error: {error}"),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: AppErrorCode::Internal,
            message: message.into(),
        }
    }

    pub fn into_local_envelope(&self) -> LocalErrorEnvelope {
        LocalErrorEnvelope {
            error: LocalErrorBody {
                code: self.code.as_str().to_string(),
                message: self.message.clone(),
            },
        }
    }

    pub fn into_openai_envelope(&self) -> OpenAiErrorEnvelope {
        OpenAiErrorEnvelope {
            error: OpenAiErrorBody {
                code: self.code.as_str().to_string(),
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
