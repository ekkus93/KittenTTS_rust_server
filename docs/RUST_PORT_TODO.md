# RUST_PORT_TODO.md

## Goal

Implement a Rust replacement for `KittenTTS_server` using `kitten_tts_rs` as the synthesis backend while preserving the current HTTP behavior and deployment model as closely as practical.

This TODO is organized into phases, tasks, and subtasks so GitHub Copilot can work through it incrementally.

---

## Assumptions

- The Python server behavior in `KittenTTS_server-master` is the compatibility target.
- `kitten_tts_rs-main` is the starting point for the Rust synthesis backend.
- The first implementation should favor correctness and compatibility over performance.
- The first implementation should preserve `espeak-ng` as a dependency.
- The first implementation should **not** attempt real incremental streaming synthesis.

---

## Phase 0 - Project setup and structure

### 0.1 Create the Rust server crate
- [x] Create a new Rust crate for the HTTP service, for example `kittentts_server_rs`
- [ ] Decide whether the service is:
  - [x] a standalone crate with a separate `kitten_tts_rs` dependency boundary (path dependency wiring still pending)
  - [ ] a Cargo workspace sibling crate
  - [ ] or a single repo with vendored backend code
- [x] Add a basic `Cargo.toml` with server dependencies
- [x] Add a `src/` layout that separates:
  - [x] config
  - [x] models
  - [x] routes
  - [x] services
  - [x] backend adapter
  - [x] middleware
  - [x] error handling

### 0.2 Choose and wire foundational crates
- [x] Add HTTP framework crate, preferably `axum`
- [x] Add `tokio`
- [x] Add `serde` and `serde_json`
- [x] Add `tracing` and `tracing-subscriber`
- [x] Add `uuid`
- [x] Add `thiserror` or `anyhow`
- [x] Add `tower` / `tower-http` as needed
- [ ] Add audio serialization support crate(s) if needed

### 0.3 Add repository hygiene
- [ ] Add `.gitignore` entries as needed
- [x] Add a basic `README.md` for the Rust server crate
- [x] Add sample config file(s)
- [x] Add a sample `.env` or docs note for env overrides
- [ ] Add CI placeholders if appropriate

### 0.4 Establish initial module skeleton
- [x] Create `src/main.rs`
- [x] Create `src/app_state.rs`
- [x] Create `src/config.rs`
- [x] Create `src/error.rs`
- [x] Create `src/logging.rs`
- [x] Create `src/models/api.rs`
- [x] Create `src/models/internal.rs`
- [x] Create `src/routes/health.rs`
- [x] Create `src/routes/voices.rs`
- [x] Create `src/routes/tts.rs`
- [x] Create `src/services/audio.rs`
- [x] Create `src/services/synth.rs`
- [x] Create `src/services/voices.rs`
- [x] Create `src/backend/kitten.rs`
- [x] Create `src/middleware/auth.rs`
- [x] Create `src/middleware/request_context.rs`

### 0.5 Define initial acceptance for Phase 0
- [x] Project compiles
- [x] Server binary starts
- [x] Basic empty router can respond on a port
- [x] Code structure is in place for later phases

---

## Phase 1 - Port configuration loading and validation

### 1.1 Port settings model from Python
- [x] Create Rust settings struct matching Python fields:
  - [x] `host`
  - [x] `port`
  - [x] `auth_enabled`
  - [x] `local_api_key`
  - [x] `default_voice_id`
  - [x] `default_model_id`
  - [x] `voice_map`
  - [x] `output_format`
  - [x] `sample_rate`
  - [x] `channel_layout`
  - [x] `log_level`
  - [x] `strict_mode`

### 1.2 Implement config file loading
- [x] Load settings from JSON config file
- [x] Validate that config root is an object
- [ ] Produce clear errors for:
  - [x] missing file
  - [x] invalid JSON
  - [x] invalid field types
  - [ ] invalid enum-like field values

### 1.3 Implement environment overrides
- [x] Mirror the Python env prefix behavior
- [x] Parse string overrides
- [x] Parse integer overrides
- [x] Parse boolean overrides
- [x] Parse JSON object override for `VOICE_MAP`
- [ ] Merge config file + environment overrides with the same effective precedence as Python

### 1.4 Validate settings values
- [ ] Validate `port`
- [x] Validate `sample_rate`
- [x] Validate `channel_layout` is `mono` or `stereo`
- [x] Validate `log_level`
- [ ] Validate `output_format` handling assumptions for current v1 support

### 1.5 Add config tests
- [ ] Test defaults
- [ ] Test config file loading
- [ ] Test environment overrides
- [ ] Test invalid boolean parsing
- [ ] Test invalid integer parsing
- [ ] Test invalid JSON map parsing
- [ ] Test invalid `channel_layout`
- [ ] Test invalid `log_level`

### 1.6 Acceptance for Phase 1
- [ ] Server can boot from config file
- [ ] Server can boot from env overrides
- [ ] Invalid config fails clearly

---

## Phase 2 - Port request/response models and error envelopes

### 2.1 Create public API models
- [ ] Create Rust equivalent of `VoiceSettings`
- [ ] Create Rust equivalent of `TTSRequest`
- [ ] Create Rust equivalent of `OpenAISpeechRequest`
- [x] Create Rust equivalent of health response
- [ ] Create Rust equivalent of voice descriptor
- [ ] Create Rust equivalent of voice list response

### 2.2 Create internal request model
- [x] Create `InternalSynthesisRequest`
- [ ] Include:
  - [x] `text`
  - [x] `voice_id`
  - [x] `model_id`
  - [x] `speed`
  - [x] `output_format`
  - [x] `streaming`

### 2.3 Create error types
- [x] Create app error type analogous to `ShimError`
- [ ] Support:
  - [x] HTTP status code
  - [x] stable app error code string
  - [x] message
  - [ ] optional details
- [x] Create local JSON error envelope
- [x] Create OpenAI-style error envelope

### 2.4 Port strict-mode request validation behavior
- [ ] For ElevenLabs-like routes, support permissive mode
- [ ] In strict mode, reject unsupported top-level fields
- [ ] In strict mode, reject unsupported `voice_settings` fields
- [ ] Preserve current behavior that only `speed` materially affects synthesis

### 2.5 Add model/error tests
- [ ] Test valid request parsing
- [ ] Test empty text handling
- [ ] Test OpenAI request validation
- [ ] Test strict-mode unsupported-field rejection
- [ ] Test local error envelope shape
- [ ] Test OpenAI error envelope shape

### 2.6 Acceptance for Phase 2
- [ ] JSON contracts are stable
- [x] Error serialization exists for both compatibility styles

---

## Phase 3 - Port voice logic

### 3.1 Port voice resolution behavior
- [ ] Implement case-insensitive available-voice lookup
- [ ] Implement alias lookup via `voice_map`
- [ ] Implement default fallback behavior
- [ ] Preserve Python semantics:
  - [ ] alias map first
  - [ ] direct case-insensitive match second
  - [ ] fallback to default otherwise

### 3.2 Port voice descriptor generation
- [ ] Build ElevenLabs-shaped descriptors from backend voice list
- [ ] Include alias metadata
- [ ] Lowercase canonical `voice_id`
- [ ] Preserve description format closely

### 3.3 Decide and document unknown-voice behavior
- [ ] Preserve current Python fallback-to-default behavior unless strict mode requires otherwise
- [ ] Add explicit tests so this does not drift silently

### 3.4 Add voice tests
- [ ] Test alias preference
- [ ] Test case-insensitive direct match
- [ ] Test default fallback when missing
- [ ] Test default fallback for unknown voice
- [ ] Test alias metadata appears in voice descriptors

### 3.5 Acceptance for Phase 3
- [ ] Voice resolution is compatible with Python behavior
- [ ] `/v1/voices` can be implemented using this layer

---

## Phase 4 - Build the backend adapter around `kitten_tts_rs`

### 4.1 Create backend abstraction
- [ ] Define a synthesizer trait or equivalent interface
- [ ] Include methods for:
  - [ ] listing voices
  - [ ] synthesizing audio
- [ ] Create backend result type that includes:
  - [ ] audio waveform or PCM buffer
  - [ ] resolved backend voice

### 4.2 Wrap `kitten_tts_rs`
- [ ] Create a backend adapter in `src/backend/kitten.rs`
- [ ] Load model
- [ ] Load voices
- [ ] Expose voice listing
- [ ] Expose synthesis

### 4.3 Preserve server compatibility behavior inside or around the backend
- [ ] Ensure the HTTP server uses `clean_text = false`
- [ ] Ensure style-row selection matches Python:
  - [ ] use character count of the chunk text
  - [ ] not token count
- [ ] Ensure output trim behavior matches Python
- [ ] Ensure long-text chunking behavior matches Python closely
- [ ] Ensure punctuation enforcement for chunking matches Python closely
- [ ] Ensure speed prior behavior remains compatible

### 4.4 Explicitly audit `kitten_tts_rs` behavior against Python KittenTTS
- [ ] Compare voice alias mapping behavior
- [ ] Compare phonemizer command usage
- [ ] Compare token-ID generation logic
- [ ] Compare style vector selection logic
- [ ] Compare output trimming
- [ ] Compare multi-chunk generation path
- [ ] Compare preprocessing default behavior
- [ ] Add comments or docs for any intentional differences

### 4.5 Decide how to apply compatibility fixes
- [ ] Option A: patch/fork `kitten_tts_rs`
- [ ] Option B: keep upstream backend mostly intact and add a compatibility wrapper
- [ ] Choose one and document it in code comments / README

### 4.6 Add backend initialization checks
- [ ] Fail clearly if ONNX model missing
- [ ] Fail clearly if `voices.npz` missing
- [ ] Fail clearly if `espeak-ng` missing
- [ ] Decide whether these are startup-fatal or exposed as unavailable runtime state
- [ ] Prefer fail-fast unless there is a strong reason not to

### 4.7 Add backend tests
- [ ] Unit test voice-name resolution in the backend layer
- [ ] Unit test style-row selection compatibility helper
- [ ] Unit test text chunking helper
- [ ] Unit test punctuation enforcement helper
- [ ] Unit test behavior when `espeak-ng` missing, if feasible
- [ ] Add at least one integration-style test for actual synthesis if test environment allows model access

### 4.8 Acceptance for Phase 4
- [ ] Rust backend can produce speech for a known voice
- [ ] Critical compatibility differences are addressed explicitly
- [ ] Startup behavior around missing dependencies is clear

---

## Phase 5 - Port audio pipeline

### 5.1 Port float waveform -> PCM conversion
- [ ] Accept mono float waveform
- [ ] Validate expected shape
- [ ] Clip to `[-1.0, 1.0]`
- [ ] Convert to signed 16-bit PCM
- [ ] Preserve 24 kHz mono as the backend-native format

### 5.2 Port audio normalization
- [ ] Port mono -> stereo duplication
- [ ] Port stereo -> mono averaging if needed
- [ ] Port linear resampling logic
- [ ] Validate sample-rate and channel arguments
- [ ] Preserve behavior as close to Python `app/audio.py` as practical

### 5.3 Port audio serialization
- [ ] Implement WAV serialization
- [ ] Implement raw PCM passthrough
- [ ] Keep serialization separate from routes

### 5.4 Add audio tests
- [ ] Test float clipping
- [ ] Test PCM conversion
- [ ] Test mono -> stereo conversion
- [ ] Test stereo -> mono conversion
- [ ] Test linear resampling
- [ ] Test WAV bytes are valid and parseable
- [ ] Test PCM passthrough length

### 5.5 Acceptance for Phase 5
- [ ] Rust service can produce valid audio bytes from backend output
- [ ] WAV responses are playable
- [ ] PCM responses are correct for OpenAI-compatible route

---

## Phase 6 - Port auth and middleware

### 6.1 Port API-key extraction logic
- [ ] Extract `xi-api-key`
- [ ] Extract `Authorization: Bearer ...`
- [ ] Detect conflicting values when both headers are present
- [ ] Return the effective API key when they agree

### 6.2 Port route auth policy
- [ ] Keep `/healthz` public
- [ ] Protect `/v1...` routes
- [ ] Respect `auth_enabled`
- [ ] Respect configured local API key

### 6.3 Port request context middleware
- [ ] Generate request IDs
- [ ] Track selected voice in request extensions/state
- [ ] Track text length in request extensions/state
- [ ] Measure latency
- [ ] Attach `X-Request-Id` to all responses

### 6.4 Port request logging behavior
- [ ] Log method
- [ ] Log path
- [ ] Log status code
- [ ] Log latency
- [ ] Log selected voice if available
- [ ] Log text length if available
- [ ] Log app error code if available

### 6.5 Add auth/middleware tests
- [ ] Test public path access
- [ ] Test protected path with auth disabled
- [ ] Test protected path with missing auth when enabled
- [ ] Test `xi-api-key` success
- [ ] Test bearer token success
- [ ] Test conflicting header failure
- [ ] Test `X-Request-Id` header present

### 6.6 Acceptance for Phase 6
- [ ] Auth behavior matches Python server expectations
- [ ] Request ID and logging metadata are wired up

---

## Phase 7 - Port HTTP routes

### 7.1 Implement `/healthz`
- [x] Return status
- [x] Return engine name
- [x] Return engine version
- [x] Return model-loaded flag
- [x] Return default voice
- [x] Return output format
- [x] Return sample rate
- [x] Return channel layout

### 7.2 Implement `/v1/voices`
- [ ] Query backend voices
- [ ] Build descriptors
- [ ] Return stable JSON shape

### 7.3 Implement `POST /v1/text-to-speech`
- [ ] Parse ElevenLabs-like request
- [ ] Normalize into `InternalSynthesisRequest`
- [ ] Use default voice if none specified
- [ ] Synthesize
- [ ] Normalize audio
- [ ] Serialize WAV
- [ ] Set `X-Output-Format`

### 7.4 Implement `POST /v1/text-to-speech/{voice_id}`
- [ ] Parse request
- [ ] Normalize into internal request with explicit voice
- [ ] Resolve voice
- [ ] Synthesize
- [ ] Normalize audio
- [ ] Serialize response
- [ ] Set response headers

### 7.5 Implement `POST /v1/text-to-speech/{voice_id}/stream`
- [ ] Preserve current pseudo-stream behavior
- [ ] Do **not** attempt incremental synthesis in v1
- [ ] Synthesize entire audio first
- [ ] Chunk already-generated bytes into the response body
- [ ] Preserve output-format negotiation behavior closely

### 7.6 Implement `POST /v1/audio/speech`
- [ ] Parse OpenAI-compatible request
- [ ] Normalize into internal request
- [ ] Support `wav`
- [ ] Support `pcm`
- [ ] Use OpenAI-style errors for this route
- [ ] Set response headers appropriately

### 7.7 Port output-format negotiation
- [ ] Port normal output-format negotiation behavior
- [ ] Port stream-route format negotiation behavior
- [ ] Preserve strict-mode handling for unsupported formats
- [ ] Preserve fallback behavior when strict mode is off

### 7.8 Port exception/error handling
- [ ] Map internal app errors to local JSON envelope
- [ ] Map OpenAI route errors to OpenAI envelope
- [ ] Map validation failures
- [ ] Map unexpected failures

### 7.9 Add route tests
- [x] Test `/healthz`
- [ ] Test `/v1/voices`
- [ ] Test `/v1/text-to-speech` success
- [ ] Test `/v1/text-to-speech/{voice_id}` success
- [ ] Test `/v1/text-to-speech/{voice_id}` unknown voice behavior
- [ ] Test empty text returns 400
- [ ] Test `/v1/audio/speech` WAV success
- [ ] Test `/v1/audio/speech` PCM success
- [ ] Test `/v1/audio/speech` validation failure shape
- [ ] Test strict-mode unsupported output format
- [ ] Test stream route returns chunked body
- [ ] Test `X-Output-Format`
- [ ] Test `X-Request-Id`

### 7.10 Acceptance for Phase 7
- [ ] All public routes exist
- [ ] Routes behave compatibly enough to replace the Python server for current clients

---

## Phase 8 - Startup wiring and app state

### 8.1 Build app state
- [x] Store validated settings
- [ ] Store synthesizer service/backend
- [x] Store engine metadata
- [x] Make app state accessible to routes and middleware

### 8.2 Port runtime initialization behavior
- [x] Load settings at startup
- [x] Initialize logging before serving
- [ ] Initialize backend before serving
- [ ] Decide how to represent model-loaded vs unavailable backend
- [ ] Prefer explicit startup failure for required dependencies

### 8.3 Add startup tests if practical
- [ ] Test startup with valid config
- [ ] Test startup failure with missing model
- [ ] Test startup failure with missing voices file
- [ ] Test startup failure with invalid config

### 8.4 Acceptance for Phase 8
- [ ] Server boot path is deterministic and clear
- [ ] Required dependencies are verified early

---

## Phase 9 - Docker and deployment parity

### 9.1 Create Dockerfile for Rust server
- [ ] Build the Rust binary in a builder stage
- [ ] Package only runtime dependencies in final stage
- [ ] Install `espeak-ng` in runtime image
- [ ] Copy model and voices assets into the image or define a mounted-path strategy
- [ ] Expose the configured port
- [ ] Add healthcheck if appropriate

### 9.2 Port compose support
- [ ] Create or update a compose file
- [ ] Support config file mounting
- [ ] Support env overrides
- [ ] Support model/voices mounting if not baked into the image

### 9.3 Port systemd support
- [ ] Create a sample systemd unit file for the Rust binary
- [ ] Support config path
- [ ] Support environment file or direct env vars
- [ ] Set reasonable restart policy

### 9.4 Add deployment docs
- [ ] Document local run instructions
- [ ] Document Docker build/run instructions
- [ ] Document compose instructions
- [ ] Document systemd install instructions
- [ ] Document `espeak-ng` requirement

### 9.5 Acceptance for Phase 9
- [ ] Service can run locally
- [ ] Service can run in Docker
- [ ] Service can run under systemd

---

## Phase 10 - Regression tests and polish

### 10.1 Build a regression checklist
- [ ] Compare route paths
- [ ] Compare status codes
- [ ] Compare response headers
- [ ] Compare error envelopes
- [ ] Compare voice listing shape
- [ ] Compare auth behavior
- [ ] Compare output format negotiation
- [ ] Compare OpenAI route behavior

### 10.2 Add compatibility-focused tests
- [ ] Test style-row selection compatibility helper specifically
- [ ] Test `clean_text=false` behavior path is used by the HTTP service
- [ ] Test unknown voice fallback behavior explicitly
- [ ] Test selected voice is logged / carried in request context
- [ ] Test text length is recorded in request context
- [ ] Test OpenAI route returns OpenAI-shaped auth errors

### 10.3 Optional manual validation tasks
- [ ] Run the Rust service and Python service side by side
- [ ] Send the same request payloads to both
- [ ] Compare:
  - [ ] status codes
  - [ ] response headers
  - [ ] JSON shape
  - [ ] WAV validity
  - [ ] perceived voice selection behavior
- [ ] Note any intentional differences in a compatibility section of the README

### 10.4 Cleanup and documentation
- [ ] Add code comments around compatibility-sensitive logic
- [ ] Add README notes on architecture
- [ ] Add README notes on model assets and dependencies
- [ ] Add README notes on limitations of pseudo-streaming
- [ ] Add README notes on non-goals

### 10.5 Acceptance for Phase 10
- [ ] The Rust port is understandable, test-covered, and documented
- [ ] Known compatibility differences are explicit, not accidental

---

## Suggested file targets during implementation

These are not mandatory, but they are recommended.

### Core app files
- [x] `src/main.rs`
- [x] `src/app_state.rs`
- [x] `src/config.rs`
- [x] `src/error.rs`
- [x] `src/logging.rs`

### Middleware
- [x] `src/middleware/auth.rs`
- [x] `src/middleware/request_context.rs`

### Models
- [x] `src/models/api.rs`
- [x] `src/models/internal.rs`

### Services
- [x] `src/services/audio.rs`
- [x] `src/services/synth.rs`
- [x] `src/services/voices.rs`

### Backend adapter
- [x] `src/backend/kitten.rs`

### Routes
- [x] `src/routes/health.rs`
- [x] `src/routes/voices.rs`
- [x] `src/routes/tts.rs`

### Deployment files
- [ ] `Dockerfile`
- [ ] `compose.yaml`
- [x] `config/settings.example.json`
- [ ] `config/systemd/kittentts-server-rs.service`

---

## Definition of done

The port is done when all of the following are true:

- [x] Rust server starts successfully with valid config
- [x] `/healthz` works
- [ ] `/v1/voices` works
- [ ] `/v1/text-to-speech` works
- [ ] `/v1/text-to-speech/{voice_id}` works
- [ ] `/v1/text-to-speech/{voice_id}/stream` works as pseudo-streaming
- [ ] `/v1/audio/speech` works with both WAV and PCM
- [ ] Auth behavior matches current expectations
- [ ] Error envelopes match current route compatibility expectations
- [ ] Voice alias resolution matches current expectations
- [ ] Audio output is valid and playable
- [ ] `espeak-ng` is handled correctly in runtime packaging
- [ ] Docker deployment works
- [ ] systemd deployment works
- [ ] Compatibility-sensitive behavior is explicitly tested
