use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use futures_util::stream;
use serde_json::Value;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::middleware::request_context::{
    set_error_code, set_selected_voice, set_text_length, SharedRequestContext,
};
use crate::models::api::{OpenAiSpeechRequest, TtsRequest};
use crate::models::internal::InternalSynthesisRequest;
use crate::services::audio::{
    float_audio_to_pcm, normalize_audio, serialize_pcm_s16le, serialize_wav,
};
use crate::services::voices::resolve_voice;

const SUPPORTED_STREAM_SAMPLE_RATES: [u32; 4] = [16_000, 22_050, 24_000, 44_100];
const STREAM_CHUNK_SIZE: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
struct StreamFormat {
    container: &'static str,
    header_value: String,
    media_type: &'static str,
    sample_rate: u32,
    channels: u16,
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/text-to-speech", post(text_to_speech_default))
        .route("/v1/text-to-speech/{voice_id}", post(text_to_speech))
        .route(
            "/v1/text-to-speech/{voice_id}/stream",
            post(text_to_speech_stream),
        )
        .route("/v1/audio/speech", post(openai_text_to_speech))
}

async fn text_to_speech_default(
    State(state): State<AppState>,
    Extension(request_context): Extension<SharedRequestContext>,
    Json(payload): Json<TtsRequest>,
) -> Result<Response, AppError> {
    build_tts_response(None, payload, state, &request_context)
}

async fn text_to_speech(
    State(state): State<AppState>,
    Path(voice_id): Path<String>,
    Extension(request_context): Extension<SharedRequestContext>,
    Json(payload): Json<TtsRequest>,
) -> Result<Response, AppError> {
    build_tts_response(Some(voice_id), payload, state, &request_context)
}

async fn text_to_speech_stream(
    State(state): State<AppState>,
    Path(voice_id): Path<String>,
    Extension(request_context): Extension<SharedRequestContext>,
    Json(payload): Json<TtsRequest>,
) -> Result<Response, AppError> {
    build_streaming_tts_response(voice_id, payload, state, &request_context)
}

async fn openai_text_to_speech(
    State(state): State<AppState>,
    Extension(request_context): Extension<SharedRequestContext>,
    Json(payload): Json<Value>,
) -> Response {
    match parse_openai_request(payload)
        .and_then(|request| build_openai_speech_response(request, state, &request_context))
    {
        Ok(response) => response,
        Err(error) => {
            set_error_code(&request_context, error.code.as_str());
            (error.status, Json(error.into_openai_envelope())).into_response()
        }
    }
}

fn parse_openai_request(payload: Value) -> Result<OpenAiSpeechRequest, AppError> {
    serde_json::from_value(payload)
        .map_err(|err| AppError::validation(format!("Invalid OpenAI speech request: {err}")))
}

fn build_tts_response(
    requested_voice_id: Option<String>,
    payload: TtsRequest,
    state: AppState,
    request_context: &SharedRequestContext,
) -> Result<Response, AppError> {
    let internal_request = payload.to_internal_request(
        requested_voice_id.as_deref(),
        state.settings.strict_mode,
        false,
    )?;
    let negotiated_output_format = negotiate_output_format(
        internal_request.output_format.as_deref(),
        &state.settings.output_format,
        state.settings.strict_mode,
    )?;
    let source_audio = synthesize_audio(&internal_request, &state, request_context)?;
    let normalized_audio = normalize_audio(
        &source_audio,
        state.settings.sample_rate,
        state.settings.output_channels(),
    )?;
    let wav_bytes = serialize_wav(&normalized_audio)?;

    build_binary_response(wav_bytes, "audio/wav", &negotiated_output_format)
}

fn build_streaming_tts_response(
    requested_voice_id: String,
    payload: TtsRequest,
    state: AppState,
    request_context: &SharedRequestContext,
) -> Result<Response, AppError> {
    let internal_request = payload.to_internal_request(
        Some(requested_voice_id.as_str()),
        state.settings.strict_mode,
        true,
    )?;
    let stream_format = negotiate_stream_format(
        internal_request.output_format.as_deref(),
        state.settings.sample_rate,
        state.settings.output_channels(),
        state.settings.strict_mode,
    )?;
    let source_audio = synthesize_audio(&internal_request, &state, request_context)?;
    let normalized_audio = normalize_audio(
        &source_audio,
        stream_format.sample_rate,
        stream_format.channels,
    )?;

    // Compatibility note: v1 keeps the Python shim's pseudo-streaming contract.
    // The full payload is synthesized first and only then emitted as chunks.
    let response_bytes = match stream_format.container {
        "pcm" => serialize_pcm_s16le(&normalized_audio)?,
        _ => serialize_wav(&normalized_audio)?,
    };

    build_streaming_response(response_bytes, &stream_format)
}

fn build_openai_speech_response(
    payload: OpenAiSpeechRequest,
    state: AppState,
    request_context: &SharedRequestContext,
) -> Result<Response, AppError> {
    let internal_request = payload.to_internal_request()?;
    let source_audio = synthesize_audio(&internal_request, &state, request_context)?;
    let normalized_audio = normalize_audio(
        &source_audio,
        state.settings.sample_rate,
        state.settings.output_channels(),
    )?;

    match internal_request.output_format.as_deref() {
        Some("pcm") => {
            let response_bytes = serialize_pcm_s16le(&normalized_audio)?;
            build_binary_response(response_bytes, "audio/pcm", "pcm")
        }
        _ => {
            let response_bytes = serialize_wav(&normalized_audio)?;
            build_binary_response(response_bytes, "audio/wav", "wav")
        }
    }
}

fn synthesize_audio(
    internal_request: &InternalSynthesisRequest,
    state: &AppState,
    request_context: &SharedRequestContext,
) -> Result<crate::services::audio::AudioBuffer, AppError> {
    let available_voices = state.synth_runtime.synthesizer().list_voices();
    // Keep resolution in the HTTP layer so alias-first, direct-match, and
    // default-fallback behavior stays aligned with the Python compatibility target.
    let resolved_voice = resolve_voice(
        internal_request.voice_id.as_deref(),
        &state.settings.voice_map,
        &available_voices,
        &state.settings.default_voice_id,
    );

    let mut backend_request = internal_request.clone();
    backend_request.voice_id = Some(resolved_voice);
    set_selected_voice(
        request_context,
        backend_request.voice_id.clone().unwrap_or_default(),
    );
    set_text_length(request_context, internal_request.text.len());

    let synth_result = state
        .synth_runtime
        .synthesizer()
        .synthesize(&backend_request)?;

    float_audio_to_pcm(&synth_result.audio)
}

fn negotiate_output_format(
    requested_output_format: Option<&str>,
    configured_output_format: &str,
    strict_mode: bool,
) -> Result<String, AppError> {
    let normalized_output_format = normalize_output_format(requested_output_format);
    let Some(output_format) = normalized_output_format else {
        return Ok(configured_output_format.to_string());
    };

    if output_format == "wav" || output_format.starts_with("wav_") {
        return Ok("wav".to_string());
    }

    if strict_mode {
        return Err(AppError::validation(format!(
            "Unsupported output_format: {output_format}"
        )));
    }

    Ok(configured_output_format.to_string())
}

fn normalize_output_format(output_format: Option<&str>) -> Option<String> {
    let normalized = output_format?.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

fn negotiate_stream_format(
    requested_output_format: Option<&str>,
    settings_sample_rate: u32,
    settings_channels: u16,
    strict_mode: bool,
) -> Result<StreamFormat, AppError> {
    let normalized_output_format = normalize_output_format(requested_output_format);
    let Some(output_format) = normalized_output_format else {
        return Ok(StreamFormat {
            container: "wav",
            header_value: "wav".to_string(),
            media_type: "audio/wav",
            sample_rate: settings_sample_rate,
            channels: settings_channels,
        });
    };

    if let Some(stream_format) = supported_stream_format(&output_format, settings_sample_rate) {
        return Ok(StreamFormat {
            channels: settings_channels,
            ..stream_format
        });
    }

    if strict_mode {
        return Err(AppError::validation(format!(
            "Unsupported output_format: {output_format}"
        )));
    }

    Ok(StreamFormat {
        container: "wav",
        header_value: "wav".to_string(),
        media_type: "audio/wav",
        sample_rate: settings_sample_rate,
        channels: settings_channels,
    })
}

fn supported_stream_format(output_format: &str, settings_sample_rate: u32) -> Option<StreamFormat> {
    let (container, sample_rate) = match output_format.split_once('_') {
        Some((container_token, sample_rate_token)) => {
            let sample_rate = sample_rate_token.parse::<u32>().ok()?;
            if !SUPPORTED_STREAM_SAMPLE_RATES.contains(&sample_rate) {
                return None;
            }
            let container = match container_token {
                "wav" => "wav",
                "pcm" => "pcm",
                _ => return None,
            };
            (container, sample_rate)
        }
        None => match output_format {
            "wav" => ("wav", settings_sample_rate),
            "pcm" => ("pcm", settings_sample_rate),
            _ => return None,
        },
    };

    let media_type = match container {
        "wav" => "audio/wav",
        "pcm" => "audio/pcm",
        _ => return None,
    };

    let header_value = if output_format.contains('_') {
        output_format.to_string()
    } else {
        container.to_string()
    };

    Some(StreamFormat {
        container,
        header_value,
        media_type,
        sample_rate,
        channels: 0,
    })
}

fn build_streaming_response(
    bytes: Vec<u8>,
    stream_format: &StreamFormat,
) -> Result<Response, AppError> {
    let content_type = HeaderValue::from_static(stream_format.media_type);
    let output_format_value = HeaderValue::from_str(&stream_format.header_value)
        .map_err(|err| AppError::internal(format!("invalid X-Output-Format header: {err}")))?;

    let chunks = bytes
        .chunks(STREAM_CHUNK_SIZE)
        .map(Bytes::copy_from_slice)
        .map(Ok::<_, std::convert::Infallible>)
        .collect::<Vec<_>>();
    let mut response = Response::new(Body::from_stream(stream::iter(chunks)));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(CONTENT_TYPE, content_type);
    response
        .headers_mut()
        .insert("X-Output-Format", output_format_value);
    Ok(response)
}

fn build_binary_response(
    bytes: Vec<u8>,
    media_type: &'static str,
    output_format: &str,
) -> Result<Response, AppError> {
    let content_length = HeaderValue::from_str(&bytes.len().to_string())
        .map_err(|err| AppError::internal(format!("invalid Content-Length header: {err}")))?;
    let content_type = HeaderValue::from_static(media_type);
    let output_format_value = HeaderValue::from_str(output_format)
        .map_err(|err| AppError::internal(format!("invalid X-Output-Format header: {err}")))?;

    let mut response = Response::new(axum::body::Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(CONTENT_TYPE, content_type);
    response
        .headers_mut()
        .insert(CONTENT_LENGTH, content_length);
    response
        .headers_mut()
        .insert("X-Output-Format", output_format_value);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::middleware::request_context::RequestContext;
    use crate::services::synth::{test_runtime, FloatAudioBuffer, SynthResult, Synthesizer};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use futures_util::StreamExt;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    #[derive(Clone)]
    struct FakeSynthesizer {
        available_voices: Vec<String>,
        waveform: Vec<f32>,
        last_voice: Arc<Mutex<Option<String>>>,
    }

    impl Synthesizer for FakeSynthesizer {
        fn list_voices(&self) -> Vec<String> {
            self.available_voices.clone()
        }

        fn synthesize(&self, request: &InternalSynthesisRequest) -> Result<SynthResult, AppError> {
            *self.last_voice.lock().unwrap() = request.voice_id.clone();

            Ok(SynthResult {
                audio: FloatAudioBuffer {
                    waveform: self.waveform.clone(),
                    sample_rate: 24_000,
                    channels: 1,
                },
                voice: request.voice_id.clone().unwrap_or_default(),
            })
        }
    }

    fn test_state(
        settings: crate::config::Settings,
        last_voice: Arc<Mutex<Option<String>>>,
    ) -> AppState {
        test_state_with_waveform(settings, last_voice, vec![0.0, 0.25, -0.25, 0.5])
    }

    fn test_state_with_waveform(
        settings: crate::config::Settings,
        last_voice: Arc<Mutex<Option<String>>>,
        waveform: Vec<f32>,
    ) -> AppState {
        let synthesizer = FakeSynthesizer {
            available_voices: vec!["Jasper".to_string(), "Bella".to_string()],
            waveform,
            last_voice,
        };

        AppState::from_runtime(settings, test_runtime(synthesizer))
    }

    fn test_request_context() -> SharedRequestContext {
        Arc::new(Mutex::new(RequestContext {
            request_id: "test-request-id".to_string(),
            selected_voice: None,
            text_length: None,
            error_code: None,
        }))
    }

    #[tokio::test]
    async fn text_to_speech_default_returns_wav_response() {
        let last_voice = Arc::new(Mutex::new(None));
        let state = test_state(crate::config::Settings::default(), Arc::clone(&last_voice));
        let app = crate::routes::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"text": "hello"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "audio/wav");
        assert_eq!(response.headers()["X-Output-Format"], "wav");

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[0..4], b"RIFF");
        assert_eq!(*last_voice.lock().unwrap(), Some("Jasper".to_string()));
    }

    #[test]
    fn synthesize_audio_records_resolved_voice_in_request_context() {
        let last_voice = Arc::new(Mutex::new(None));
        let settings = crate::config::Settings {
            voice_map: BTreeMap::from([("Narrator".to_string(), "Bella".to_string())]),
            ..crate::config::Settings::default()
        };
        let state = test_state(settings, Arc::clone(&last_voice));
        let request_context = test_request_context();
        let request = InternalSynthesisRequest {
            text: "hello request context".to_string(),
            voice_id: Some("Narrator".to_string()),
            model_id: None,
            speed: 1.0,
            output_format: Some("wav".to_string()),
            streaming: false,
        };

        synthesize_audio(&request, &state, &request_context).unwrap();

        assert_eq!(*last_voice.lock().unwrap(), Some("Bella".to_string()));
        assert_eq!(
            request_context.lock().unwrap().selected_voice.as_deref(),
            Some("Bella")
        );
    }

    #[test]
    fn synthesize_audio_records_text_length_in_request_context() {
        let text = "hello request context";
        let last_voice = Arc::new(Mutex::new(None));
        let state = test_state(crate::config::Settings::default(), last_voice);
        let request_context = test_request_context();
        let request = InternalSynthesisRequest {
            text: text.to_string(),
            voice_id: None,
            model_id: None,
            speed: 1.0,
            output_format: Some("wav".to_string()),
            streaming: false,
        };

        synthesize_audio(&request, &state, &request_context).unwrap();

        assert_eq!(
            request_context.lock().unwrap().text_length,
            Some(text.len())
        );
    }

    #[tokio::test]
    async fn text_to_speech_path_route_uses_alias_mapping() {
        let last_voice = Arc::new(Mutex::new(None));
        let settings = crate::config::Settings {
            voice_map: BTreeMap::from([("Narrator".to_string(), "Bella".to_string())]),
            ..crate::config::Settings::default()
        };
        let state = test_state(settings, Arc::clone(&last_voice));
        let app = crate::routes::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech/Narrator")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"text": "hello alias"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(*last_voice.lock().unwrap(), Some("Bella".to_string()));
    }

    #[tokio::test]
    async fn text_to_speech_unknown_voice_falls_back_to_default() {
        let last_voice = Arc::new(Mutex::new(None));
        let state = test_state(crate::config::Settings::default(), Arc::clone(&last_voice));
        let app = crate::routes::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech/UnknownVoice")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"text": "hello fallback"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(*last_voice.lock().unwrap(), Some("Jasper".to_string()));
    }

    #[tokio::test]
    async fn empty_text_returns_bad_request() {
        let last_voice = Arc::new(Mutex::new(None));
        let state = test_state(crate::config::Settings::default(), last_voice);
        let app = crate::routes::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"text": "   "}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn openai_speech_returns_wav_response() {
        let last_voice = Arc::new(Mutex::new(None));
        let state = test_state(crate::config::Settings::default(), Arc::clone(&last_voice));
        let app = crate::routes::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/audio/speech")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "model": "tts-1",
                            "voice": "Bella",
                            "input": "hello wav",
                            "response_format": "wav"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "audio/wav");
        assert_eq!(response.headers()["X-Output-Format"], "wav");

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[0..4], b"RIFF");
        assert_eq!(*last_voice.lock().unwrap(), Some("Bella".to_string()));
    }

    #[tokio::test]
    async fn openai_speech_returns_pcm_response() {
        let last_voice = Arc::new(Mutex::new(None));
        let state = test_state(crate::config::Settings::default(), Arc::clone(&last_voice));
        let app = crate::routes::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/audio/speech")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "model": "tts-1",
                            "voice": "Bella",
                            "input": "hello pcm",
                            "response_format": "pcm"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "audio/pcm");
        assert_eq!(response.headers()["X-Output-Format"], "pcm");

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(!body.is_empty());
        assert_ne!(&body[0..4.min(body.len())], b"RIFF");
        assert_eq!(*last_voice.lock().unwrap(), Some("Bella".to_string()));
    }

    #[tokio::test]
    async fn stream_route_returns_wav_response_with_stream_format() {
        let last_voice = Arc::new(Mutex::new(None));
        let app = crate::routes::build_router(test_state_with_waveform(
            crate::config::Settings::default(),
            Arc::clone(&last_voice),
            vec![0.1; 20_000],
        ));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech/Bella/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"text": "hello stream", "output_format": "wav_16000"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "audio/wav");
        assert_eq!(response.headers()["X-Output-Format"], "wav_16000");
        assert!(response.headers().get(CONTENT_LENGTH).is_none());

        let mut body_stream = response.into_body().into_data_stream();
        let mut chunk_count = 0;
        let mut collected = Vec::new();
        while let Some(chunk) = body_stream.next().await {
            let chunk = chunk.unwrap();
            chunk_count += 1;
            collected.extend_from_slice(&chunk);
        }
        assert!(
            chunk_count > 1,
            "expected more than one streamed body chunk"
        );
        let body = Bytes::from(collected);
        assert_eq!(&body[0..4], b"RIFF");
        assert_eq!(
            u32::from_le_bytes([body[24], body[25], body[26], body[27]]),
            16_000
        );
        assert_eq!(*last_voice.lock().unwrap(), Some("Bella".to_string()));
    }

    #[tokio::test]
    async fn stream_route_returns_pcm_response_with_stream_format() {
        let last_voice = Arc::new(Mutex::new(None));
        let app = crate::routes::build_router(test_state(
            crate::config::Settings::default(),
            Arc::clone(&last_voice),
        ));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech/Bella/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"text": "hello stream", "output_format": "pcm_16000"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "audio/pcm");
        assert_eq!(response.headers()["X-Output-Format"], "pcm_16000");

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(!body.is_empty());
        assert_ne!(&body[0..4.min(body.len())], b"RIFF");
        assert_eq!(body.len(), 6);
        assert_eq!(*last_voice.lock().unwrap(), Some("Bella".to_string()));
    }

    #[tokio::test]
    async fn strict_mode_rejects_unsupported_output_format() {
        let last_voice = Arc::new(Mutex::new(None));
        let settings = crate::config::Settings {
            strict_mode: true,
            ..crate::config::Settings::default()
        };
        let state = test_state(settings, last_voice);
        let app = crate::routes::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"text": "hello", "output_format": "mp3"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_text = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_text.contains("Unsupported output_format"));
    }

    #[tokio::test]
    async fn strict_mode_rejects_unsupported_stream_output_format() {
        let last_voice = Arc::new(Mutex::new(None));
        let settings = crate::config::Settings {
            strict_mode: true,
            ..crate::config::Settings::default()
        };
        let app = crate::routes::build_router(test_state(settings, last_voice));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech/Bella/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"text": "hello", "output_format": "mp3"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_text = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_text.contains("Unsupported output_format"));
    }

    #[tokio::test]
    async fn openai_validation_failure_returns_openai_error_shape() {
        let last_voice = Arc::new(Mutex::new(None));
        let state = test_state(crate::config::Settings::default(), last_voice);
        let app = crate::routes::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/audio/speech")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "model": "bad-model",
                            "voice": "Bella",
                            "input": "hello"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_text = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_text.starts_with('{'), "unexpected body: {body_text}");
        let body_json: serde_json::Value = serde_json::from_str(&body_text).unwrap();
        assert!(body_json.get("error").is_some());
        assert_eq!(body_json["error"]["type"], "validation_error");
    }
}
