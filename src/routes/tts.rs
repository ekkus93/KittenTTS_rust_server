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
use crate::error::{AppError, AppErrorCode};
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
    run_blocking_tts(move || build_tts_response(None, payload, state, &request_context)).await
}

async fn text_to_speech(
    State(state): State<AppState>,
    Path(voice_id): Path<String>,
    Extension(request_context): Extension<SharedRequestContext>,
    Json(payload): Json<TtsRequest>,
) -> Result<Response, AppError> {
    run_blocking_tts(move || build_tts_response(Some(voice_id), payload, state, &request_context))
        .await
}

async fn text_to_speech_stream(
    State(state): State<AppState>,
    Path(voice_id): Path<String>,
    Extension(request_context): Extension<SharedRequestContext>,
    Json(payload): Json<TtsRequest>,
) -> Result<Response, AppError> {
    run_blocking_tts(move || {
        build_streaming_tts_response(voice_id, payload, state, &request_context)
    })
    .await
}

async fn openai_text_to_speech(
    State(state): State<AppState>,
    Extension(request_context): Extension<SharedRequestContext>,
    Json(payload): Json<Value>,
) -> Response {
    // Retain request metadata for the HTTP error envelope after the blocking
    // closure takes ownership of its synthesis request context.
    let error_context = request_context.clone();
    match run_blocking_tts(move || {
        parse_openai_request(payload)
            .and_then(|request| build_openai_speech_response(request, state, &request_context))
    })
    .await
    {
        Ok(response) => response,
        Err(error) => {
            set_error_code(&error_context, error.code.as_str());
            (error.status, Json(error.into_openai_envelope())).into_response()
        }
    }
}

// Keep synchronous ONNX generation and PCM serialization off Tokio worker threads.
// The backend still serializes its single model instance via Mutex<KittenTTS>;
// offloading protects other HTTP handlers but does not parallelize inference.
// Once started, spawn_blocking may continue even after its HTTP future is dropped.
async fn run_blocking_tts<F>(operation: F) -> Result<Response, AppError>
where
    F: FnOnce() -> Result<Response, AppError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|err| AppError::internal(format!("speech synthesis task failed: {err}")))?
}

fn parse_openai_request(payload: Value) -> Result<OpenAiSpeechRequest, AppError> {
    // The Python/FastAPI compatibility target maps request-schema failures to
    // HTTP 422 with a fixed public message. Keep Serde's internal diagnostics
    // out of the client-visible envelope; valid requests still follow the
    // existing domain-validation path (e.g. empty input is HTTP 400).
    serde_json::from_value(payload).map_err(|_| {
        AppError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            AppErrorCode::Validation,
            "Request validation failed",
        )
    })
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
        state.settings.strict_mode,
    )?;
    let source_audio = synthesize_audio(&internal_request, &state, request_context)?;
    let normalized_audio = normalize_audio(
        &source_audio,
        stream_format.sample_rate,
        state.settings.output_channels(),
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

fn unsupported_output_format(output_format: &str) -> AppError {
    let mut details = std::collections::BTreeMap::new();
    details.insert(
        "output_format".to_string(),
        Value::String(output_format.to_string()),
    );
    AppError::new(
        StatusCode::BAD_REQUEST,
        AppErrorCode::UnsupportedOutputFormat,
        format!("Unsupported output_format: {output_format}"),
    )
    .with_details(details)
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
        return Err(unsupported_output_format(&output_format));
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
    strict_mode: bool,
) -> Result<StreamFormat, AppError> {
    let normalized_output_format = normalize_output_format(requested_output_format);
    let Some(output_format) = normalized_output_format else {
        return Ok(StreamFormat {
            container: "wav",
            header_value: "wav".to_string(),
            media_type: "audio/wav",
            sample_rate: settings_sample_rate,
        });
    };

    if let Some(stream_format) = supported_stream_format(&output_format, settings_sample_rate) {
        return Ok(stream_format);
    }

    if strict_mode {
        return Err(unsupported_output_format(&output_format));
    }

    Ok(StreamFormat {
        container: "wav",
        header_value: "wav".to_string(),
        media_type: "audio/wav",
        sample_rate: settings_sample_rate,
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
    let content_length = HeaderValue::from(bytes.len());
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

    #[test]
    fn blocking_synthesis_does_not_starve_single_tokio_worker() {
        use std::sync::{mpsc, Condvar};
        use std::time::Duration;

        struct GatedSynthesizer {
            entered: mpsc::Sender<()>,
            release: Arc<(Mutex<bool>, Condvar)>,
        }

        impl Synthesizer for GatedSynthesizer {
            fn list_voices(&self) -> Vec<String> {
                vec!["Jasper".to_string()]
            }

            fn synthesize(
                &self,
                request: &InternalSynthesisRequest,
            ) -> Result<SynthResult, AppError> {
                self.entered.send(()).expect("gate receiver exists");
                let (lock, ready) = &*self.release;
                let mut released = lock.lock().expect("gate is not poisoned");
                while !*released {
                    released = ready.wait(released).expect("gate is not poisoned");
                }
                Ok(SynthResult {
                    audio: FloatAudioBuffer {
                        waveform: vec![0.0, 0.2, -0.2, 0.4],
                        sample_rate: 24_000,
                        channels: 1,
                    },
                    voice: request.voice_id.clone().unwrap_or_default(),
                })
            }
        }

        let (entered_tx, entered_rx) = mpsc::channel();
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let state = AppState::from_runtime(
            crate::config::Settings {
                auth_enabled: true,
                local_api_key: Some("secret".to_string()),
                ..crate::config::Settings::default()
            },
            test_runtime(GatedSynthesizer {
                entered: entered_tx,
                release: Arc::clone(&release),
            }),
        );
        let app = crate::routes::build_router(state);

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("one-worker runtime");
        let (speech_tx, speech_rx) = mpsc::channel();
        let speech_app = app.clone();
        runtime.spawn(async move {
            let response = speech_app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/text-to-speech")
                        .header("content-type", "application/json")
                        .header("xi-api-key", "secret")
                        .body(Body::from(r#"{"text":"blocking inference"}"#))
                        .expect("request"),
                )
                .await
                .expect("speech response");
            let _ = speech_tx.send(response.status());
        });

        // The test thread runs outside the runtime: a blocked Tokio worker
        // cannot accidentally prevent us from releasing the fake inference.
        let entered = entered_rx.recv_timeout(Duration::from_secs(5));
        let (health_tx, health_rx) = mpsc::channel();
        let health_app = app.clone();
        runtime.spawn(async move {
            let response = health_app
                .oneshot(
                    Request::builder()
                        .uri("/healthz")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("health response");
            let _ = health_tx.send(response.status());
        });
        let health = health_rx.recv_timeout(Duration::from_secs(3));

        // Invalid input and unauthenticated requests should also be answered
        // without waiting for an ongoing model inference to finish.
        let (invalid_tx, invalid_rx) = mpsc::channel();
        let invalid_app = app.clone();
        runtime.spawn(async move {
            let response = invalid_app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/text-to-speech")
                        .header("content-type", "application/json")
                        .header("xi-api-key", "secret")
                        .body(Body::from(r#"{"text":""}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            let _ = invalid_tx.send(response.status());
        });
        let invalid = invalid_rx.recv_timeout(Duration::from_secs(3));

        let (unauth_tx, unauth_rx) = mpsc::channel();
        runtime.spawn(async move {
            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/v1/voices")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let _ = unauth_tx.send(response.status());
        });
        let unauth = unauth_rx.recv_timeout(Duration::from_secs(3));

        // Always release the fake inference, including on timeout, so a
        // regression does not hang the test process or Tokio shutdown.
        {
            let (lock, ready) = &*release;
            *lock.lock().expect("gate is not poisoned") = true;
            ready.notify_all();
        }
        let speech = speech_rx.recv_timeout(Duration::from_secs(5));
        assert!(entered.is_ok(), "synthesis never started: {entered:?}");
        assert_eq!(health.ok(), Some(StatusCode::OK), "health request starved");
        assert_eq!(
            invalid.ok(),
            Some(StatusCode::BAD_REQUEST),
            "validation starved"
        );
        assert_eq!(unauth.ok(), Some(StatusCode::UNAUTHORIZED), "auth starved");
        assert_eq!(speech.ok(), Some(StatusCode::OK), "speech did not finish");
    }

    #[test]
    fn cancelled_http_request_does_not_corrupt_synthesis_state() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{mpsc, Condvar};
        use std::time::Duration;

        struct CancelAwareSynth {
            started: mpsc::Sender<()>,
            released: Arc<(Mutex<bool>, Condvar)>,
            model_lock: Mutex<()>,
            calls: AtomicUsize,
        }

        impl Synthesizer for CancelAwareSynth {
            fn list_voices(&self) -> Vec<String> {
                vec!["Jasper".to_string()]
            }

            fn synthesize(
                &self,
                request: &InternalSynthesisRequest,
            ) -> Result<SynthResult, AppError> {
                let _model_guard = self.model_lock.lock().unwrap();
                if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    self.started.send(()).expect("test receiver is available");
                    let (lock, ready) = &*self.released;
                    let mut released = lock.lock().unwrap();
                    while !*released {
                        released = ready.wait(released).unwrap();
                    }
                }
                Ok(SynthResult {
                    audio: FloatAudioBuffer {
                        waveform: vec![0.0, 0.2, -0.2, 0.4],
                        sample_rate: 24_000,
                        channels: 1,
                    },
                    voice: request.voice_id.clone().unwrap_or_default(),
                })
            }
        }

        let (started_tx, started_rx) = mpsc::channel();
        let released = Arc::new((Mutex::new(false), Condvar::new()));
        let app = crate::routes::build_router(AppState::from_runtime(
            crate::config::Settings::default(),
            test_runtime(CancelAwareSynth {
                started: started_tx,
                released: Arc::clone(&released),
                model_lock: Mutex::new(()),
                calls: AtomicUsize::new(0),
            }),
        ));

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let cancelled_app = app.clone();
        let cancelled = runtime.spawn(async move {
            cancelled_app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/text-to-speech")
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"text":"cancelled request"}"#))
                        .unwrap(),
                )
                .await
        });

        let first_started = started_rx.recv_timeout(Duration::from_secs(5));
        cancelled.abort();
        {
            let (lock, ready) = &*released;
            *lock.lock().unwrap() = true;
            ready.notify_all();
        }

        let (second_tx, second_rx) = mpsc::channel();
        runtime.spawn(async move {
            let result = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/text-to-speech")
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"text":"following request"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            let _ = second_tx.send(result.status());
        });
        let second_status = second_rx.recv_timeout(Duration::from_secs(5));
        assert!(first_started.is_ok(), "first synthesis never started");
        assert_eq!(
            second_status.ok(),
            Some(StatusCode::OK),
            "backend unusable after cancellation"
        );
    }

    #[tokio::test]
    async fn panicking_blocking_task_becomes_internal_application_error() {
        let error = run_blocking_tts(|| panic!("deliberate test panic"))
            .await
            .expect_err("join failure should not produce a successful response");
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(error.message.contains("speech synthesis task failed"));
    }

    #[tokio::test]
    async fn blocking_backend_panic_uses_route_specific_error_envelopes() {
        struct PanickingSynthesizer;

        impl Synthesizer for PanickingSynthesizer {
            fn list_voices(&self) -> Vec<String> {
                vec!["Jasper".to_string()]
            }

            fn synthesize(
                &self,
                _request: &InternalSynthesisRequest,
            ) -> Result<SynthResult, AppError> {
                panic!("deliberate backend panic")
            }
        }

        let app = crate::routes::build_router(AppState::from_runtime(
            crate::config::Settings::default(),
            test_runtime(PanickingSynthesizer),
        ));

        let local = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/text-to-speech")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"hello"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(local.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(local.headers().get("X-Request-Id").is_some());
        let local_body = to_bytes(local.into_body(), usize::MAX).await.unwrap();
        let local_json: serde_json::Value = serde_json::from_slice(&local_body).unwrap();
        assert!(local_json["error"]["code"].is_string());

        let openai = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/audio/speech")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"model":"tts-1","voice":"Jasper","input":"hello"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(openai.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(openai.headers().get("X-Request-Id").is_some());
        let openai_body = to_bytes(openai.into_body(), usize::MAX).await.unwrap();
        let openai_json: serde_json::Value = serde_json::from_slice(&openai_body).unwrap();
        assert!(openai_json["error"]["type"].is_string());
    }

    #[tokio::test]
    async fn concurrent_tts_requests_keep_voice_audio_and_request_ids_distinct() {
        struct VoiceTaggedSynthesizer;

        impl Synthesizer for VoiceTaggedSynthesizer {
            fn list_voices(&self) -> Vec<String> {
                vec!["Jasper".to_string(), "Bella".to_string()]
            }

            fn synthesize(
                &self,
                request: &InternalSynthesisRequest,
            ) -> Result<SynthResult, AppError> {
                let tone = if request.voice_id.as_deref() == Some("Bella") {
                    0.7
                } else {
                    0.2
                };
                Ok(SynthResult {
                    audio: FloatAudioBuffer {
                        waveform: vec![0.0, tone, -tone, 0.0],
                        sample_rate: 24_000,
                        channels: 1,
                    },
                    voice: request.voice_id.clone().unwrap_or_default(),
                })
            }
        }

        let app = crate::routes::build_router(AppState::from_runtime(
            crate::config::Settings::default(),
            test_runtime(VoiceTaggedSynthesizer),
        ));
        let jasper_request = Request::builder()
            .method("POST")
            .uri("/v1/text-to-speech/Jasper")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"text":"Jasper request"}"#))
            .unwrap();
        let bella_request = Request::builder()
            .method("POST")
            .uri("/v1/text-to-speech/Bella")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"text":"Bella request"}"#))
            .unwrap();
        let (jasper_result, bella_result) = tokio::join!(
            app.clone().oneshot(jasper_request),
            app.oneshot(bella_request)
        );
        let jasper = jasper_result.unwrap();
        let bella = bella_result.unwrap();
        assert_eq!(jasper.status(), StatusCode::OK);
        assert_eq!(bella.status(), StatusCode::OK);
        assert_eq!(jasper.headers()[CONTENT_TYPE], "audio/wav");
        assert_eq!(bella.headers()[CONTENT_TYPE], "audio/wav");
        let jasper_id = jasper.headers()["X-Request-Id"]
            .to_str()
            .unwrap()
            .to_string();
        let bella_id = bella.headers()["X-Request-Id"]
            .to_str()
            .unwrap()
            .to_string();
        assert_ne!(jasper_id, bella_id);
        let jasper_bytes = to_bytes(jasper.into_body(), usize::MAX).await.unwrap();
        let bella_bytes = to_bytes(bella.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&jasper_bytes[0..4], b"RIFF");
        assert_eq!(&bella_bytes[0..4], b"RIFF");
        // The sample after the zero-valued first frame encodes the chosen voice.
        assert_ne!(&jasper_bytes[46..48], &bella_bytes[46..48]);
    }

    #[test]
    fn normalize_output_format_trims_lowercases_and_rejects_blank_values() {
        assert_eq!(
            normalize_output_format(Some("  WAV_16000  ")),
            Some("wav_16000".to_string())
        );
        assert_eq!(normalize_output_format(Some("   ")), None);
        assert_eq!(normalize_output_format(None), None);
    }

    #[test]
    fn negotiate_output_format_accepts_wav_variants_and_falls_back_when_not_strict() {
        assert_eq!(
            negotiate_output_format(Some(" WAV_22050 "), "wav", false).unwrap(),
            "wav"
        );
        assert_eq!(
            negotiate_output_format(Some("pcm"), "wav", false).unwrap(),
            "wav"
        );
        assert_eq!(negotiate_output_format(None, "wav", false).unwrap(), "wav");
    }

    #[test]
    fn negotiate_output_format_rejects_unsupported_values_in_strict_mode() {
        let error = negotiate_output_format(Some("pcm"), "wav", true).unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("Unsupported output_format"));
    }

    #[test]
    fn supported_stream_format_parses_default_and_explicit_sample_rates() {
        assert_eq!(
            supported_stream_format("wav", 24_000),
            Some(StreamFormat {
                container: "wav",
                header_value: "wav".to_string(),
                media_type: "audio/wav",
                sample_rate: 24_000,
            })
        );
        assert_eq!(
            supported_stream_format("pcm_16000", 24_000),
            Some(StreamFormat {
                container: "pcm",
                header_value: "pcm_16000".to_string(),
                media_type: "audio/pcm",
                sample_rate: 16_000,
            })
        );
    }

    #[test]
    fn supported_stream_format_rejects_unknown_containers_and_sample_rates() {
        assert_eq!(supported_stream_format("mp3", 24_000), None);
        assert_eq!(supported_stream_format("wav_12345", 24_000), None);
        assert_eq!(supported_stream_format("flac_16000", 24_000), None);
    }

    #[test]
    fn negotiate_stream_format_defaults_and_falls_back_when_not_strict() {
        assert_eq!(
            negotiate_stream_format(None, 24_000, false).unwrap(),
            StreamFormat {
                container: "wav",
                header_value: "wav".to_string(),
                media_type: "audio/wav",
                sample_rate: 24_000,
            }
        );
        assert_eq!(
            negotiate_stream_format(Some("mp3"), 24_000, false).unwrap(),
            StreamFormat {
                container: "wav",
                header_value: "wav".to_string(),
                media_type: "audio/wav",
                sample_rate: 24_000,
            }
        );
    }

    #[test]
    fn negotiate_stream_format_rejects_unsupported_values_in_strict_mode() {
        let error = negotiate_stream_format(Some("pcm_12345"), 24_000, true).unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("Unsupported output_format"));
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
        let json_body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json_body,
            json!({
                "error": {
                    "code": "unsupported_output_format",
                    "message": "Unsupported output_format: mp3",
                    "details": {"output_format": "mp3"}
                }
            })
        );
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

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(response.headers().contains_key("X-Request-Id"));
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            body_json,
            json!({
                "error": {
                    "code": "validation_error",
                    "message": "Request validation failed",
                    "type": "invalid_request_error"
                }
            })
        );
    }

    #[tokio::test]
    async fn openai_domain_validation_retains_bad_request_and_missing_input() {
        let last_voice = Arc::new(Mutex::new(None));
        let app =
            crate::routes::build_router(test_state(crate::config::Settings::default(), last_voice));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/audio/speech")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "model": "tts-1",
                            "voice": "Jasper",
                            "input": "  "
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["error"]["code"], "missing_input");
        assert_eq!(parsed["error"]["type"], "missing_input");
        assert_eq!(parsed["error"]["message"], "input is required");
    }
}
