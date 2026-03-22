# RUST_PORT.md

## Purpose

Port `KittenTTS_server` from Python/FastAPI to Rust, using `kitten_tts_rs` as the synthesis backend while preserving the current HTTP behavior and deployment model as closely as practical.

This document is meant to be implementation context for GitHub Copilot. The matching task list lives in `RUST_PORT_TODO.md`.

---

## Repos examined

This port plan is based on the following codebases:

- `KittenTTS_server-master/`
- `kitten_tts_rs-main/`
- `KittenTTS-main/`

### Source-of-truth policy

For this port, treat the repos as follows:

- **HTTP/API behavior source of truth:** `KittenTTS_server-master`
- **TTS backend behavior source of truth:** `KittenTTS-main`
- **Rust backend starting point:** `kitten_tts_rs-main`

That means the Rust server should primarily preserve the public behavior of `KittenTTS_server`, while using `kitten_tts_rs` to replace the Python TTS runtime.

---

## High-level goal

The finished Rust service should be a drop-in or near-drop-in replacement for the current Python server for these use-cases:

- local ElevenLabs-compatible TTS API
- local OpenAI-compatible `/v1/audio/speech` TTS API
- Docker deployment
- systemd service deployment
- optional API-key auth
- voice alias mapping
- WAV and PCM output
- pseudo-stream endpoint behavior

---

## Non-goals for v1

Do **not** expand scope in the initial port. In version 1, avoid:

- true incremental / low-latency streaming synthesis
- MP3 / Opus / AAC output
- SSML
- live voice cloning
- hot-reloading models or voices
- replacing `espeak-ng`
- changing tokenization / pronunciation behavior intentionally
- changing route contracts unless required by Rust framework differences
- adding a database, queue, or distributed worker layer

The first goal is a faithful Rust port, not a redesign.

---

## Recommended implementation strategy

Build a **new Rust HTTP server crate** that wraps `kitten_tts_rs`, rather than trying to embed Python or do a hybrid server.

Recommended layout:

- create a new Rust service crate, for example `kittentts_server_rs`
- either:
  - depend on `kitten_tts_rs` as a local path dependency, or
  - copy/fork its code into an internal module if compatibility changes are required quickly
- preserve the existing route surface and config behavior from the Python server

If the repo structure is flexible, a Cargo workspace is a good fit:

```text
workspace/
  kitten_tts_rs/
  kittentts_server_rs/
```

---

## Why `espeak-ng` is still needed

The KittenTTS model does **not** consume raw English text directly. It consumes a phoneme/IPA-like sequence converted into token IDs.

Current flow:

```text
raw text
  -> phonemizer (`espeak-ng`)
  -> phoneme symbols
  -> token IDs
  -> ONNX model + style embedding + speed
  -> waveform
```

Important implication:

- the model is **not** doing audio-to-audio style transfer from `espeak-ng`
- `espeak-ng` is only providing pronunciation symbols
- the model is conditioned on:
  - phoneme token IDs
  - a selected style vector from `voices.npz`
  - speed

So for the Rust server, `espeak-ng` should remain a required system dependency in v1.

---

## Phase 4 audit findings

The backend audit compares `KittenTTS/kittentts/onnx_model.py` and `KittenTTS/kittentts/preprocess.py`
against the forked `kitten_tts_rs` sources used by this server.

### Audit summary

- Voice alias mapping: partially compatible by design.
  Python backend accepts configured `voice_aliases` and then requires an internal voice ID.
  The Rust fork accepts configured aliases first, then built-in friendly names, then internal IDs.
  This is an intentional extension to support the Rust server's compatibility layer and CLI ergonomics.

- Phonemizer command usage: intentionally not byte-for-byte identical.
  Python uses `phonemizer.backend.EspeakBackend(language="en-us", preserve_punctuation=True, with_stress=True)`.
  The Rust fork shells out directly to `espeak-ng --ipa -q --sep= -v en-us`.
  This keeps the Rust backend self-contained, but punctuation/stress output can differ subtly at the phoneme boundary.

- Token-ID generation logic: compatible in structure.
  Both implementations tokenize phoneme text with a word-or-punctuation split, join tokens with spaces, map characters through the same symbol table, and add leading `0`, trailing `10`, and final `0` markers.

- Style vector selection logic: now explicitly compatible.
  Python uses `min(len(text), rows - 1)` where `text` is the already chunked text string.
  The Rust fork now mirrors that rule using character count instead of token count.

- Output trimming: compatible.
  Python trims the last 5000 samples from each generated chunk with `audio = outputs[0][..., :-5000]`.
  The Rust fork trims `min(5000, audio.len())` samples from the end of each chunk.

- Multi-chunk generation path: compatible in structure.
  Both implementations split on sentence boundaries first, then split oversized sentences on whitespace boundaries, ensure terminal punctuation per chunk, synthesize each chunk independently, and concatenate the outputs in order.

- Preprocessing default behavior: intentionally diverges at the backend boundary, but the server path preserves Python server behavior.
  The Python backend defaults `clean_text=True`, while the Rust server deliberately calls the backend with `clean_text = false` to match `KittenTTS_server`.
  The Rust fork still retains optional preprocessing support for direct backend/CLI use.

### Intentional differences to keep visible

- The Rust fork supports friendly built-in voice names directly inside the backend, while Python backend internals only require configured aliases plus internal voice IDs.
- The Rust fork avoids Python's `phonemizer` package and calls `espeak-ng` directly, so exact punctuation/stress output is not guaranteed to be identical even though the same system dependency is used.
- The Rust fork keeps optional preprocessing for standalone use, but the Rust HTTP server must continue to force `clean_text = false` for compatibility with `KittenTTS_server`.

---

## Current Python server architecture

Primary Python files:

- `app/main.py`
- `app/api.py`
- `app/auth.py`
- `app/audio.py`
- `app/config.py`
- `app/errors.py`
- `app/models.py`
- `app/synth.py`
- `app/voices.py`

### What each file currently does

#### `app/main.py`
- creates the FastAPI app
- loads settings
- initializes synthesis runtime
- installs middleware
- adds exception handlers
- attaches request IDs and logs request metadata

#### `app/api.py`
- defines routes
- parses request payloads
- converts external requests into `InternalSynthesisRequest`
- resolves voices
- calls synthesis backend
- normalizes audio
- serializes audio responses
- negotiates stream output format

#### `app/auth.py`
- route auth policy
- API-key extraction from:
  - `xi-api-key`
  - `Authorization: Bearer ...`
- conflict detection if both headers disagree

#### `app/audio.py`
- audio normalization
- mono/stereo conversion
- linear resampling
- WAV serialization
- PCM passthrough

#### `app/config.py`
- config model and validation
- config file loading
- environment overrides
- settings cache / reload

#### `app/errors.py`
- structured app errors
- OpenAI-style error envelope helpers
- local shim error envelope helpers

#### `app/models.py`
- public request/response models
- internal synthesis request model

#### `app/synth.py`
- backend abstraction
- KittenTTS wrapper
- float waveform -> PCM s16le conversion
- backend runtime creation
- fallback unavailable synthesizer

#### `app/voices.py`
- voice alias resolution
- voice descriptor list building

---

## Current public API surface to preserve

Routes in the Python server:

- `GET /healthz`
- `GET /v1/voices`
- `POST /v1/text-to-speech`
- `POST /v1/text-to-speech/{voice_id}`
- `POST /v1/text-to-speech/{voice_id}/stream`
- `POST /v1/audio/speech`

### Route behavior summary

#### `GET /healthz`
Returns status and server config metadata.

#### `GET /v1/voices`
Returns ElevenLabs-shaped voice descriptors derived from local backend voices and configured aliases.

#### `POST /v1/text-to-speech`
Default-voice TTS route.

#### `POST /v1/text-to-speech/{voice_id}`
Explicit-voice ElevenLabs-style route.

#### `POST /v1/text-to-speech/{voice_id}/stream`
Pseudo-stream route. This is **not** true streaming synthesis. The Python server synthesizes the full response first, then emits it in chunks.

#### `POST /v1/audio/speech`
OpenAI-compatible TTS route.

---

## Current request models to preserve

### ElevenLabs-like request

Python model: `TTSRequest`

Fields:
- `text: str`
- `model_id: Optional[str]`
- `voice_settings: Optional[VoiceSettings]`
- `output_format: Optional[str]`

`voice_settings` currently accepts:
- `speed`
- `stability`
- `similarity_boost`
- `style`
- `use_speaker_boost`

Only `speed` currently matters for synthesis. The other fields are compatibility shims.

### OpenAI-compatible request

Python model: `OpenAISpeechRequest`

Fields:
- `model`
- `voice`
- `input`
- `response_format`
- `speed`

Current allowed `model` values:
- `gpt-4o-mini-tts`
- `tts-1`
- `tts-1-hd`

Current allowed `response_format` values:
- `wav`
- `pcm`

---

## Current response behavior to preserve

### Common response behavior
- attach `X-Request-Id`
- attach `X-Output-Format` for audio routes
- return playable WAV when container is `wav`
- return raw PCM s16le bytes when OpenAI route requests `pcm`

### Error style
The server uses two error styles:
- local shim JSON error envelope for most routes
- OpenAI-style error envelope for `/v1/audio/speech`

This distinction should be preserved.

---

## Current auth behavior to preserve

Protected paths:
- all `/v1...` routes

Public path:
- `/healthz`

Auth sources:
- `xi-api-key`
- `Authorization: Bearer <token>`

Behavior:
- if auth disabled, allow requests
- if enabled, require configured local API key
- if both headers present and different, reject with auth error

OpenAI route auth failures should return OpenAI-style errors.

---

## Current config behavior to preserve

Config file example: `config/settings.example.json`

Fields:
- `host`
- `port`
- `auth_enabled`
- `local_api_key`
- `default_voice_id`
- `default_model_id`
- `voice_map`
- `output_format`
- `sample_rate`
- `channel_layout`
- `log_level`
- `strict_mode`

Environment variable prefix in Python:
- `KITTENTTS_SERVER_...`

Key environment override behaviors to preserve:
- typed parsing for int/bool/string values
- JSON parsing for `VOICE_MAP`
- config file + environment merging
- validation with explicit errors

---

## Current voice behavior to preserve

Python `resolve_voice(...)` behavior:

1. if explicit `requested_voice_id` is in configured `voice_map`, use mapped value
2. else if requested voice matches an available backend voice case-insensitively, use that
3. else fall back to default voice
4. if no requested voice, use default voice

Important implication:
- unknown voices generally fall back to default at the voice-resolution layer

Do **not** silently “improve” this behavior without checking compatibility expectations.

### Voice descriptors
The `/v1/voices` route exposes:
- canonical lowercase `voice_id`
- `name`
- `category`
- `description`
- labels including alias information

This behavior should be mirrored in Rust.

---

## Current audio behavior to preserve

The Python server normalizes audio after synthesis.

### Existing capabilities
- mono <-> stereo conversion
- linear resampling
- WAV serialization
- raw PCM serialization

### Expected synthesis backend format
The Python wrapper expects KittenTTS to return:
- mono audio
- float waveform
- 24 kHz sample rate

Then the server converts to PCM s16le and normalizes as needed.

---

## Current backend behavior to preserve

The Python server backend wrapper in `app/synth.py` does this:

1. list available voices from runtime
2. resolve backend voice by case-insensitive match
3. call `model.generate(text, voice=..., speed=..., clean_text=False)`
4. convert float waveform to PCM s16le
5. return 24 kHz mono audio

This is important:

- the server currently uses **`clean_text=False`**
- the Rust port should preserve that in v1

---

## What the ONNX model actually consumes

Conceptually the model input is:

```text
input_ids = pronunciation token IDs
style     = one selected style vector from `voices.npz`
speed     = scalar float
```

So the model is effectively:

```text
audio = TTS(what_to_say, who_should_say_it, how_fast)
```

Where:
- `what_to_say` = phoneme token IDs
- `who_should_say_it` = style vector
- `how_fast` = speed

---

## Important compatibility gaps between Python KittenTTS and `kitten_tts_rs`

These are the most important implementation hazards.

### 1. Style-row selection mismatch
Python `KittenTTS-main/kittentts/onnx_model.py` uses:

- `ref_id = min(len(text), rows - 1)`

Rust `kitten_tts_rs-main/src/model.rs` currently uses:

- `ref_idx = token_ids.len().min(rows - 1)`

This is a meaningful behavioral difference.

**Recommendation for v1:**
change the Rust backend or wrap it so the style row is selected using **character length of the text chunk**, matching Python.

### 2. Text cleaning mismatch
Python server calls:
- `generate(..., clean_text=False)`

Rust backend has a preprocessing path and may be more eager to normalize text.

**Recommendation for v1:**
preserve server behavior and default to `clean_text = false` for HTTP synthesis routes.

### 3. Tokenization / phoneme processing may not match exactly
Python:
- uses `phonemizer.backend.EspeakBackend(...)`
- tokenizes phonemes with `re.findall(r"\w+|[^\w\s]", text)`
- joins tokens with spaces
- uses `TextCleaner`

Rust:
- shells out to `espeak-ng`
- uses its own regex and symbol-table logic

This may be “close enough,” but it is still a possible output mismatch.
Treat this as a compatibility-sensitive area.

### 4. Trimming behavior must match
Python trims model output:
- `audio = outputs[0][..., :-5000]`

Rust backend must preserve the same trimming behavior if it is not already doing so.

### 5. Chunking behavior must match
Python `KittenTTS-main` uses chunking and punctuation enforcement:
- `chunk_text(...)`
- `ensure_punctuation(...)`

The Rust backend should preserve chunking behavior well enough for long-text synthesis.

### 6. Voice alias layers exist in two places
There are two distinct alias mechanisms:
- server-level `voice_map`
- backend-level friendly/internal voice mapping

Do not collapse these carelessly. Both layers are useful.

---

## Recommended Rust stack

Suggested crates:

- `axum` for HTTP server
- `tokio` for async runtime
- `serde`, `serde_json` for models
- `thiserror` or `anyhow` for errors
- `tracing`, `tracing-subscriber` for logging
- `uuid` for request IDs
- `hound` for WAV writing if needed
- `tower` / `tower-http` for middleware and tracing
- `dotenvy` optionally for local dev convenience
- `kitten_tts_rs` as local path dependency or internal module

This is a recommendation, not a hard requirement. The important thing is behavior preservation.

---

## Recommended Rust module layout

Suggested structure:

```text
src/
  main.rs
  app_state.rs
  config.rs
  error.rs
  logging.rs
  middleware/
    auth.rs
    request_context.rs
  models/
    api.rs
    internal.rs
  routes/
    health.rs
    voices.rs
    tts.rs
  services/
    synth.rs
    voices.rs
    audio.rs
  backend/
    kitten.rs
```

### Responsibilities

#### `main.rs`
- boot app
- load config
- initialize tracing/logging
- build app state
- build router
- start server

#### `app_state.rs`
- shared app state:
  - settings
  - synthesizer service
  - other startup metadata

#### `config.rs`
- Rust equivalent of Python `app/config.py`
- parse config file
- parse env overrides
- validate fields

#### `error.rs`
- API error types
- route error conversion
- OpenAI-style vs local-style error serialization

#### `middleware/auth.rs`
- API key extraction
- auth enforcement
- conflicting-header detection

#### `middleware/request_context.rs`
- request ID creation
- latency timing
- response header injection
- request metadata logging

#### `models/api.rs`
- external HTTP request/response structs

#### `models/internal.rs`
- `InternalSynthesisRequest`
- internal response structs

#### `routes/health.rs`
- `/healthz`

#### `routes/voices.rs`
- `/v1/voices`

#### `routes/tts.rs`
- all synthesis routes
- thin HTTP-only logic

#### `services/synth.rs`
- orchestration:
  - voice resolve
  - backend synth call
  - audio normalize
  - serialize

#### `services/voices.rs`
- voice alias resolution
- voice descriptor building

#### `services/audio.rs`
- float -> PCM conversion
- channel conversion
- resampling
- WAV and PCM serialization

#### `backend/kitten.rs`
- adapter around `kitten_tts_rs`
- compatibility-preserving behavior

---

## Recommended internal request shape

Normalize all public API routes into a single internal request model before calling synthesis logic.

Suggested internal model:

```rust
pub struct InternalSynthesisRequest {
    pub text: String,
    pub voice_id: Option<String>,
    pub model_id: Option<String>,
    pub speed: f32,
    pub output_format: Option<String>,
    pub streaming: bool,
}
```

This should mirror the Python design.

---

## Recommended request-to-audio flow

```text
HTTP request
  -> parse JSON
  -> validate and normalize request
  -> resolve requested voice
  -> call kitten backend
  -> receive mono 24k float waveform
  -> convert to PCM s16le
  -> normalize sample rate / channels
  -> serialize WAV or PCM
  -> build HTTP response
```

### Practical behavior details

#### Request parsing
- trim text/input
- reject empty strings after trimming
- default speed to `1.0`
- preserve strict-mode behavior

#### Voice resolution
- use server `voice_map`
- then match direct available backend voice case-insensitively
- then default fallback

#### Backend synth call
- preserve `clean_text = false`
- preserve style-row selection compatibility
- preserve audio trim behavior
- preserve chunking semantics for long text

#### Audio response
- set `Content-Type`
- set `Content-Length`
- set `X-Output-Format`
- set `X-Request-Id`

---

## Concurrency recommendation for v1

`kitten_tts_rs` currently exposes synthesis through mutable backend state.

For v1, favor correctness over throughput.

Recommended approach:
- wrap backend engine in `Mutex`
- run synthesis in `tokio::task::spawn_blocking`
- serialize access to the ONNX runtime if needed

Example shape:

```rust
pub struct KittenBackend {
    inner: std::sync::Mutex<KittenTTS>,
}
```

This is acceptable for v1. Throughput improvements can come later.

---

## Health endpoint expectations

The Rust `/healthz` route should include analogous data to the Python server:

- `status`
- `engine`
- `engine_version`
- `model_loaded`
- `default_voice_id`
- `output_format`
- `sample_rate`
- `channel_layout`

Startup should ideally fail early if hard requirements are missing:
- ONNX model not found
- voices file not found
- `espeak-ng` missing
- invalid config

---

## Error behavior expectations

Map internal failures to stable API errors.

### Good default mapping
- missing / empty input text -> `400`
- unsupported output format in strict mode -> `400`
- auth failure -> `401`
- missing voice after strict validation -> `404` or compatibility fallback behavior
- backend unavailable -> `503`
- synthesis runtime failure -> `500`

### Error envelope style
- `/v1/audio/speech` -> OpenAI-style
- all other shim routes -> local JSON envelope

---

## Deployment expectations

### Docker
The Rust image should:
- include the Rust server binary
- include model file(s)
- include `voices.npz`
- include `espeak-ng`
- expose configured port
- support config file and env overrides

### systemd
The Rust service should be easy to run under systemd, analogous to the current Python deployment.

---

## Test migration strategy

The Python repo already has useful behavioral tests.

Existing Python tests include:
- `test_audio.py`
- `test_auth.py`
- `test_config.py`
- `test_errors.py`
- `test_health.py`
- `test_models.py`
- `test_synth.py`
- `test_tts_basic.py`
- `test_voice_resolution.py`
- `test_voices.py`

Port these ideas, not necessarily line-for-line implementations.

### Minimum behavioral tests for Rust
- health route
- voices route
- voice alias resolution
- auth header handling
- empty text handling
- OpenAI route request validation
- WAV response validity
- PCM response validity
- pseudo-stream route returns bytes in chunks
- config parsing / override logic
- style-row compatibility wrapper behavior
- float -> PCM conversion
- mono/stereo conversion and resampling

---

## Suggested implementation checkpoints

### Checkpoint 1
Basic Rust service boots and `/healthz` works.

### Checkpoint 2
Voice listing and config loading work.

### Checkpoint 3
Backend adapter can synthesize one utterance locally from a Rust test or CLI.

### Checkpoint 4
`/v1/text-to-speech` returns valid WAV.

### Checkpoint 5
`/v1/audio/speech` returns valid WAV and PCM.

### Checkpoint 6
Pseudo-stream route works.

### Checkpoint 7
Docker and systemd deployment paths work.

### Checkpoint 8
Regression tests cover compatibility-critical behavior.

---

## Open questions to resolve during implementation

These should be settled in code, not left ambiguous:

1. Should the Rust server depend on `kitten_tts_rs` as a path dependency, or vendor/fork it for compatibility fixes?
2. Are style-row compatibility fixes implemented inside `kitten_tts_rs`, or in a wrapper layer?
3. Should unknown voices continue to fall back to default exactly as Python does, or should strict mode tighten that?
4. Should the server fail fast at startup if `espeak-ng` is missing, or expose degraded health?
5. Should route logging mirror the Python fields exactly, including selected voice and text length?

---

## Strong recommendation

For the first implementation, optimize for **behavioral compatibility**:

- preserve request/response shapes
- preserve voice-resolution behavior
- preserve `clean_text = false`
- preserve Python style-row indexing
- preserve output serialization behavior
- preserve auth and error semantics

After the Rust version is working and tested, then consider cleanup and optimization.

