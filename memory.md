## 2026-03-22T04:58:53Z - GPT-5.4 - Phase 0 scaffold established
- Added the standalone Rust server package `kittentts-server-rs` with crate identifier `kittentts_server_rs` and the locked module tree under `src/`.
- Implemented startup, logging, default/settings loading, crate exports, and a working `/healthz` route that returns server metadata.
- Added `config/settings.example.json`, updated `README.md`, and added `tests/health.rs`.
- Validated successfully with `cargo fmt --check`, `cargo check`, `cargo test`, and a live `curl http://127.0.0.1:8008/healthz` smoke test.

## 2026-03-22T05:00:21Z - GPT-5.4 - Lint and test run clean
- Ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` in `KittenTTS_rust_server`.
- All validation commands completed successfully with no lint failures and no test failures.
- Current automated test inventory remains minimal: 1 passing integration test in `tests/health.rs`, with no unit tests yet in `src/lib.rs` or `src/main.rs`.

## 2026-03-22T05:03:36Z - GPT-5.4 - RUST_PORT_TODO synced to code state
- Updated `docs/RUST_PORT_TODO.md` to reflect the completed Phase 0 scaffold, the implemented portions of Phase 1 config loading/validation, the existing Phase 2 health/internal/error model work, the implemented `/healthz` route and test, and the current startup/app-state wiring.
- Left items unchecked where behavior is not implemented yet or not verified, including exact config precedence parity, output-format validation, broader request models, backend integration, and most route/auth/audio work.

## 2026-03-22T05:04:40Z - GPT-5.4 - Current Rust repo work prepared for check-in
- The repo now contains the Phase 0 server scaffold, Rust workspace instructions, port planning docs, synchronized TODO checklist, and a clean validation run (`cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`).
- Current branch is `master`, with the local work ready to be committed and pushed to `origin/master` as one batch.

## 2026-03-22T05:12:44Z - GPT-5.4 - Phase 1 config work completed
- Completed the Rust config/settings phase: Python-matching field set, JSON config loading, env override parsing, config-over-env precedence matching the Python server, validation for port/sample rate/channel layout/log level/output format, and Python-style log-level mapping for tracing startup.
- Added `tests/config.rs` with coverage for defaults, config loading, env overrides, precedence, invalid boolean/int/JSON map parsing, and invalid channel-layout/log-level validation.
- Revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`, plus live boot checks for config-file startup, env-override startup, and invalid-config startup failure.

## 2026-03-22T05:13:52Z - GPT-5.4 - Lint and test run clean after Phase 1
- Ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` against the current Rust repo state.
- All validation commands passed successfully with no lint failures and no test failures.
- Current automated test inventory is 10 passing integration tests total: 9 in `tests/config.rs` and 1 in `tests/health.rs`.

## 2026-03-22T05:19:52Z - GPT-5.4 - Phase 2 model and error layer validated
- Added the Phase 2 API-model and error-envelope layer: `VoiceSettings`, `TtsRequest`, `OpenAiSpeechRequest`, voice descriptor/list models, richer `AppError` details support, local error details/request IDs, and OpenAI-compatible error serialization.
- Implemented strict-mode request validation for unsupported top-level and `voice_settings` fields while keeping permissive behavior available for ElevenLabs-style routes and preserving the current behavior that only `speed` materially affects synthesis.
- Revalidated successfully with `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`; the current automated inventory is 18 passing tests total (8 unit tests, 9 config integration tests, 1 health integration test).

## 2026-03-22T05:39:47Z - GPT-5.4 - Validation rerun clean on current tree
- Ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` against the current Rust repo state after the latest local edits.
- All validation commands passed successfully with no formatting failures, lint failures, or test failures.
- Current automated inventory remains 18 passing tests total: 8 unit tests, 9 config integration tests, and 1 health integration test.

## 2026-03-22T05:51:15Z - GPT-5.4 - Phase 3 voice logic completed
- Added the Phase 3 voice service layer in `src/services/voices.rs` with Python-matching resolution behavior: exact alias lookup via `voice_map`, case-insensitive direct available-voice matching, and fallback to the configured default voice.
- Added ElevenLabs-shaped voice descriptor generation with lowercase canonical `voice_id`, alias metadata in `labels`, and Python-matching description text, keeping the logic isolated in the service layer for later `/v1/voices` wiring.
- Added 5 voice unit tests covering alias preference, case-insensitive direct match, default fallback when the request is missing, fallback for unknown voices, and alias metadata in descriptors; revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` for a total of 23 passing tests.

## 2026-03-22T05:52:04Z - GPT-5.4 - Validation rerun clean after latest voice-service edits
- Ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` against the current Rust repo state after the latest local edits in `src/services/voices.rs`.
- All validation commands passed successfully with no formatting failures, lint failures, or test failures.
- Current automated inventory remains 23 passing tests total: 13 unit tests, 9 config integration tests, and 1 health integration test.

## 2026-03-22T06:10:24Z - GPT-5.4 - Phase 4 backend foundation implemented
- Added a single local path dependency from the server crate to the sibling `kitten_tts_rs` repo and implemented the Phase 4 server-side synthesis abstraction in `src/services/synth.rs`, including a typed synthesizer trait, float-audio synth result, and an explicit unavailable-backend runtime.
- Implemented the real `src/backend/kitten.rs` adapter behind the explicit `real-backend` feature, with clear checks for missing `config.json`, ONNX model files, `voices.npz`, and `espeak-ng`, and documented the choice to keep compatibility-sensitive backend fixes in the local patched backend repo.
- Patched `kitten_tts_rs` for Python-compatible style-row selection by character count and added backend-side tests for voice-name resolution, style-row selection, chunking, and punctuation helpers.
- Validation state: the Rust server package passes `cargo clippy --all-targets --no-deps -- -D warnings` and `cargo check`; the sibling `kitten_tts_rs` crate passes `cargo check --all-features`; full test/link runs that build `kitten_tts_rs` test targets are currently blocked in this Linux environment by ONNX Runtime linker errors for missing glibc C23 symbols such as `__isoc23_strtol[l|ll|ull]`.

## 2026-03-22T06:15:53Z - GPT-5.4 - Validation rerun clean on current tree
- Ran `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` from `KittenTTS_rust_server` after the latest local edits.
- All validation commands completed successfully with no formatting failures, lint failures, or test failures.
- Current automated inventory is 25 passing tests total: 15 unit tests, 9 config integration tests, and 1 health integration test.

## 2026-03-22T06:18:40Z - GPT-5.4 - Phase 4 server commit pushed
- Committed and pushed the Rust server repo changes as `010fd84 feat: add phase 4 backend foundation` on `origin/master`.
- The pushed server repo now includes the Phase 4 backend foundation in `Cargo.toml`, `src/services/synth.rs`, `src/backend/kitten.rs`, `src/error.rs`, and the synced Phase 4 TODO updates.
- The sibling backend repo `kitten_tts_rs` still has local unpublished compatibility changes in `src/model.rs` and `src/voices.rs`; they were not included in the Rust server push because they are a separate git repository.

## 2026-03-22T06:31:31Z - GPT-5.4 - Backend fork restored and pushed
- Reapplied the lost `kitten_tts_rs` compatibility changes after the repo path was replaced with a fresh fork clone, restoring the Python-compatible style-row helper/tests in `src/model.rs` and the backend voice-resolution tests in `src/voices.rs`.
- Pushed the restored backend work to the writable fork as `6495a86 fix: restore backend compatibility helpers` on `origin/main`.
- Validation for the backend fork: `cargo fmt --check` and `cargo check --all-features` passed; full `cargo test` remains blocked in this Linux environment by the same ONNX Runtime linker errors for missing glibc C23 symbols such as `__isoc23_strtol[l|ll|ull]`.

## 2026-03-22T06:34:08Z - GPT-5.4 - Phase 4 audit documented
- Completed the remaining Phase 4 audit checklist by comparing Python `kittentts/onnx_model.py` and `kittentts/preprocess.py` against the forked `kitten_tts_rs` backend for voice alias mapping, phonemizer usage, token-ID generation, style-row selection, output trimming, chunking, and preprocessing defaults.
- Added an explicit audit summary and intentional-difference notes to `docs/RUST_PORT.md`, and marked the `docs/RUST_PORT_TODO.md` Phase 4.4 items complete plus `4.8`'s compatibility-documentation acceptance item complete.
- Added code comments in the backend fork at the style-row, output-trim, and direct `espeak-ng` phonemizer boundaries so the remaining known difference is visible: direct `espeak-ng` shelling may not match Python `phonemizer` punctuation/stress output byte-for-byte.

## 2026-03-22T06:43:54Z - GPT-5.4 - Startup behavior locked to fail-fast
- Added an explicit `model_dir` setting and `KITTENTTS_SERVER_MODEL_DIR` env override so the Rust server can initialize `kitten_tts_rs` deterministically instead of guessing backend asset locations.
- Wired startup through `initialize_app_state` and `create_synth_runtime`, storing the synthesis runtime in `AppState` and failing fast before serving if the server is built without `real-backend`, if `model_dir` is missing, or if backend assets / `espeak-ng` are unavailable.
- Updated `README.md`, `config/settings.example.json`, and `docs/RUST_PORT_TODO.md` to reflect the decision; validation passed with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` for 27 passing tests total (16 unit, 10 config integration, 1 health integration).

## 2026-03-22T06:47:57Z - GPT-5.4 - Phase 4 nearly complete, final blocker is runtime environment
- Added a real `espeak-ng`-missing unit test path in `src/backend/kitten.rs` by validating a missing executable through the backend's fail-fast command check, and marked the corresponding Phase 4.7 item complete.
- Added an ignored integration-style synthesis test hook in `src/backend/kitten.rs` that can initialize the real backend and synthesize audio when `KITTENTTS_SERVER_TEST_MODEL_DIR` points to model assets in a host environment that can link and run the ONNX-backed binary.
- Current validation passed with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --tests --features real-backend`, and `cargo test`; the only remaining unchecked Phase 4 items are actual runtime synthesis acceptance and the integration-style test execution, both blocked here by missing model assets and the known ONNX Runtime linker environment constraints.

## 2026-03-22T07:05:17Z - GPT-5.4 - Rust backend now mirrors Python-style Hugging Face model download
- Added `reqwest` to the forked `kitten_tts_rs` crate and implemented `KittenTTS::from_repo`, which downloads `config.json` first and then fetches the referenced ONNX and `voices.npz` assets into a local cache.
- Cache root precedence now follows `HF_HOME`, then `XDG_CACHE_HOME`, then `HOME/.cache/huggingface/hub`, with an OS temp-directory fallback when none are set.
- Updated the Rust server startup path so `model_dir` remains an optional override; when it is absent, the server falls back to the Python default repo `KittenML/kitten-tts-nano-0.8` instead of failing immediately for a missing local asset directory.
- Validation passed with `cargo fmt --check`, `cargo check --all-features`, and `cargo check --tests --all-features` in `kitten_tts_rs`, plus `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo check --tests --features real-backend` in `KittenTTS_rust_server`.

## 2026-03-22T07:20:39Z - GPT-5.4 - Local real-backend runtime unblocked on glibc 2.35 host
- Switched `kitten_tts_rs` from `ort` default static-linking behavior to `load-dynamic` with explicit `api-24`, which avoids linking the pyke-downloaded `libonnxruntime.a` built against newer glibc C23 symbols.
- Provisioned a local ONNX Runtime shared library at `$HOME/.local/share/onnxruntime/1.24.2/libonnxruntime.so.1.24.2` and used `ORT_DYLIB_PATH` during startup.
- Moved server startup backend initialization into `tokio::task::spawn_blocking` so the first-download path can use the current blocking Hugging Face client without panicking inside the async runtime.
- Verified locally that `cargo run --features real-backend` now links and runs, downloads `config.json`, `kitten_tts_nano_v0_8.onnx`, and `voices.npz` into a fresh `HF_HOME`, and serves `/healthz` successfully with `model_loaded: true`.

## 2026-03-22T08:14:13Z - GPT-5.4 - Startup now auto-discovers local ONNX Runtime shared library
- Added server-side startup discovery for `ORT_DYLIB_PATH`: respect the existing env var first, otherwise scan `~/.local/share/onnxruntime/<version>/libonnxruntime.so*` and set `ORT_DYLIB_PATH` automatically when a local shared library is present.
- Added backend unit tests covering local shared-library discovery and the rule that an explicit `ORT_DYLIB_PATH` override is never replaced.
- Verified locally that `cargo run --features real-backend` with `KITTENTTS_SERVER_MODEL_DIR` set and `ORT_DYLIB_PATH` unset starts successfully, loads the ONNX model and voices, binds on port `8016`, and returns `/healthz` with `model_loaded: true`.

## 2026-03-22T08:15:55Z - GPT-5.4 - Startup now logs the selected ONNX Runtime library path
- Added a small info-level startup log in `src/backend/kitten.rs` that reports whether startup is using a configured or auto-discovered `ORT_DYLIB_PATH`.
- Verified with `RUST_LOG=info` that startup logs the selected path before backend initialization, for example `using auto-discovered ONNX Runtime shared library path path=/home/phil/.local/share/onnxruntime/1.24.2/libonnxruntime.so.1.24.2`.

## 2026-03-22T08:17:31Z - GPT-5.4 - ONNX Runtime startup log now includes structured source field
- Refined the startup log in `src/backend/kitten.rs` to emit a structured `source` field with values `env`, `local_discovery`, or `system_default`, instead of relying only on message wording.
- Added unit tests covering all three source-selection branches through the startup helper.
- Verified with `RUST_LOG=info` that startup now logs `selected ONNX Runtime shared library path source="local_discovery" path=/home/phil/.local/share/onnxruntime/1.24.2/libonnxruntime.so.1.24.2` before model initialization.

## 2026-03-22T08:21:40Z - GPT-5.4 - Health endpoint now exposes ONNX Runtime source metadata
- Threaded ONNX Runtime selection metadata from backend startup through `SynthRuntime` and `EngineMetadata`, so health responses can report the same structured source/path information as startup logs.
- Added `onnx_runtime_source` and `onnx_runtime_path` to `/healthz`, using `env`, `local_discovery`, or `system_default` with the resolved path when available.
- Revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` in `KittenTTS_rust_server`.

## 2026-03-22T18:20:48Z - GPT-5.4 - Ignored real-backend synthesis test passed on local host
- Ran `cargo test --features real-backend create_synth_runtime_can_generate_speech_with_real_model_assets -- --ignored --nocapture` in `KittenTTS_rust_server` with `KITTENTTS_SERVER_TEST_MODEL_DIR` set to the cached Hugging Face snapshot for `KittenML/kitten-tts-nano-0.8` and `ORT_DYLIB_PATH` set to the local shared library under `~/.local/share/onnxruntime/1.24.2/`.
- The test initialized `espeak-ng`, loaded the ONNX model and `voices.npz`, and successfully synthesized audio for the known voice `jasper`.
- This local host now has direct proof for the remaining Phase 4 synthesis acceptance items: the ignored integration-style synthesis test is runnable here, and the backend can produce speech for a known voice.

## 2026-03-22T19:03:45Z - GPT-5.4 - Phase 4 TODO synced to host-verified synthesis result
- Updated `docs/RUST_PORT_TODO.md` to mark the integration-style synthesis test item complete and to mark Phase 4 acceptance `Rust backend can produce speech for a known voice` complete.
- The checklist now matches the verified local run of `create_synth_runtime_can_generate_speech_with_real_model_assets` against the cached `KittenML/kitten-tts-nano-0.8` snapshot and local ONNX Runtime shared library.

## 2026-03-22T19:04:58Z - GPT-5.4 - Full Rust server lint and test pass is clean
- Ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` in `KittenTTS_rust_server`.
- Also ran the ignored real-backend synthesis test with `ORT_DYLIB_PATH` pointed at `~/.local/share/onnxruntime/1.24.2/libonnxruntime.so.1.24.2` and `KITTENTTS_SERVER_TEST_MODEL_DIR` pointed at the cached `KittenML/kitten-tts-nano-0.8` snapshot.
- Results were clean: 26 regular unit tests passed with 1 ignored in the all-features pass, 10 config integration tests passed, 1 health integration test passed, and the ignored real-backend synthesis test passed separately on this host.

## 2026-03-22T19:12:57Z - GPT-5.4 - Phase 5 audio service implemented and validated
- Implemented `src/services/audio.rs` with backend float-to-PCM conversion, channel normalization, linear resampling, WAV serialization, and raw PCM passthrough.
- Added unit coverage for clipping, PCM conversion, mono/stereo conversion, linear resampling, WAV container validity, and PCM passthrough length.
- Revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`; the Rust server test suite now reports 34 passing unit tests, 10 passing config integration tests, 1 passing health integration test, and 1 ignored real-backend synthesis test.

## 2026-03-22T19:22:47Z - GPT-5.4 - Non-stream TTS routes now use the Rust audio pipeline
- Implemented `src/routes/tts.rs` for `POST /v1/text-to-speech`, `POST /v1/text-to-speech/{voice_id}`, and `POST /v1/audio/speech`, wiring request normalization, voice resolution, backend synthesis, audio normalization, WAV/PCM serialization, and response headers through the Phase 5 audio service.
- Moved OpenAI request deserialization into the route handler so invalid `/v1/audio/speech` requests return the project’s OpenAI-style error envelope instead of Axum’s plain-text extractor error.
- Added route-level tests for ElevenLabs-style success paths, unknown-voice fallback, empty-text rejection, OpenAI WAV/PCM success, OpenAI validation-envelope shape, strict-mode unsupported output-format rejection, and `X-Output-Format`; `cargo test` now passes with 32 unit tests, 10 config integration tests, and 1 health integration test.

## 2026-03-22T19:27:45Z - GPT-5.4 - Voices route implemented with descriptor-service parity
- Implemented `src/routes/voices.rs` for `GET /v1/voices`, reusing `build_voice_descriptors` so the handler stays aligned with the Python compatibility shape and existing voice metadata rules.
- Added route-level coverage that exercises the full router and verifies descriptor ordering plus alias metadata in the JSON response.
- Revalidated with `cargo fmt` and `cargo test`; the Rust server suite now passes with 33 unit tests, 10 config integration tests, and 1 health integration test.

## 2026-03-22T19:35:39Z - GPT-5.4 - Stream TTS route added with format negotiation parity
- Implemented `POST /v1/text-to-speech/{voice_id}/stream` in `src/routes/tts.rs`, reusing the existing synthesis and audio-normalization pipeline while setting `streaming=true` on the internal request and supporting Python-style `wav`, `wav_<sample_rate>`, `pcm`, and `pcm_<sample_rate>` negotiation.
- Added route-level tests for stream-route WAV and PCM responses plus strict-mode rejection of unsupported stream output formats; `cargo test` now passes with 36 unit tests, 10 config integration tests, and 1 health integration test.
- The route intentionally avoids true incremental synthesis and still generates the full audio payload first; the TODO remains explicit that chunked response-body semantics are not yet implemented under the current dependency surface.

## 2026-03-22T19:41:38Z - GPT-5.4 - Auth and request-context middleware wired into the router
- Implemented `src/middleware/auth.rs` with Python-matching API-key extraction, conflicting-header detection, `/healthz` public-path handling, `/v1...` protection, and OpenAI-vs-local authentication error envelopes.
- Implemented `src/middleware/request_context.rs` with per-request UUIDs, shared request metadata for selected voice/text length/error code, `X-Request-Id` response headers, local-error request-id injection, and structured request completion logs.
- Wired both middleware layers into `build_router`, updated the TTS routes to publish voice/text metadata into the request context, and added integration coverage for public access, auth-disabled pass-through, missing auth, `xi-api-key`, bearer auth, conflicting headers, OpenAI auth error shape, and `X-Request-Id`; `cargo test` now passes with 39 unit tests, 10 config integration tests, and 8 integration tests in `tests/health.rs`.

## 2026-03-22T19:44:01Z - GPT-5.4 - Full Rust validation plus ignored real-backend test passed
- Ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` in `KittenTTS_rust_server`; all checks passed after a minimal rustfmt line-wrap fix in `tests/health.rs`.
- Standard all-features test results were clean: 49 passed, 0 failed, 1 ignored in `src/lib.rs`, plus 10 passing tests in `tests/config.rs` and 8 passing tests in `tests/health.rs`.
- Also ran the ignored real-backend synthesis test `create_synth_runtime_can_generate_speech_with_real_model_assets` with `ORT_DYLIB_PATH` set to the local ONNX Runtime shared library and `KITTENTTS_SERVER_TEST_MODEL_DIR` pointed at the cached `KittenML/kitten-tts-nano-0.8` snapshot; it passed locally.

## 2026-03-22T12:45:45-07:00 - GPT-5.4 - Route and middleware parity batch pushed
- Committed and pushed the current Rust server work to `origin/master` as `9abd0c1e3df8104599931c20475a36b9d7446f00` with subject `feat: complete route and middleware parity`.
- The working tree was clean after push.

## 2026-03-22T19:47:23Z - GPT-5.4 - TODO synced for pushed middleware and validation coverage
- Updated `docs/RUST_PORT_TODO.md` to mark the compatibility-focused validation items that are now explicitly covered by tests: style-row selection helper coverage, unknown-voice fallback coverage, and OpenAI-shaped auth error coverage.
- Left broader acceptance items unchanged where behavior is still incomplete or only partially implemented, especially stream-route chunked body semantics, request-context metadata assertions, and deployment/runtime packaging work.

## 2026-03-22T21:13:09Z - GPT-5.4 - Phase 9 deployment artifacts added and Docker validated
- Added Phase 9 deployment files in `KittenTTS_rust_server`: `Dockerfile`, `compose.yaml`, `config/systemd/kittentts-server-rs.service`, and `docker-entrypoint.sh`, plus README deployment instructions for local run, Docker, compose, and systemd.
- The Docker path keeps the sibling `kitten_tts_rs` local dependency boundary by using a named BuildKit context, bundles ONNX Runtime `1.24.2`, installs `espeak-ng`, and uses an entrypoint that prepares writable cache/config directories before dropping privileges back to `elkitten`.
- Verified locally that `docker compose config` resolves successfully, the Docker image builds successfully with `--build-context kitten_tts_rs=/home/phil/work/kitten_tts_rs`, and a container started from that image serves `GET /healthz` on the published host port with `model_loaded: true`.
- Updated `docs/RUST_PORT_TODO.md` to mark Phase 9 Docker/compose/systemd/docs tasks complete, plus `Service can run in Docker` and `Docker deployment works`; `Service can run under systemd` remains unverified.

## 2026-03-22T21:18:14Z - GPT-5.4 - README systemd docs now include uninstall flow
- Updated `README.md` to keep the documented systemd install flow and add a matching uninstall section covering `systemctl disable --now`, unit removal, daemon reload/reset, and optional cleanup of `/opt/kittentts-server-rs`, `/etc/default/kittentts-server-rs`, and the `kittentts-server` user/group.
- Live systemd installation/testing remains deferred by user choice, so the Phase 9 systemd acceptance item stays unchecked until a real host validation is run later.

## 2026-03-22T21:24:50Z - GPT-5.4 - Stream route now emits real multi-chunk pseudo-stream bodies
- Added the approved direct dependency `futures-util = "0.3"` in `Cargo.toml` and changed `src/routes/tts.rs` to build `POST /v1/text-to-speech/{voice_id}/stream` responses with `axum::body::Body::from_stream(...)` over fixed-size chunks while still synthesizing the full audio payload first.
- Added route-level verification that the WAV stream route omits `Content-Length` and yields more than one body chunk via `into_data_stream()`, while keeping the PCM stream and strict-mode rejection tests passing.
- Full validation passed after the change: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.
- Updated `docs/RUST_PORT_TODO.md` to mark the chunked pseudo-stream body task, the stream-body test item, and the Definition-of-done stream route item complete.

## 2026-03-22T21:29:26Z - GPT-5.4 - Backend path now proves clean_text is forced off
- Refactored the real backend adapter in `src/backend/kitten.rs` so the compatibility-sensitive generate call flows through a small helper that always passes `clean_text = false`.
- Added `backend_synthesis_path_forces_clean_text_false`, which captures the generate-call arguments and proves the synthesis path used by the HTTP service sends `clean_text` as `false`.
- Revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features backend_synthesis_path_forces_clean_text_false`, and `cargo test --all-features`.
- Updated `docs/RUST_PORT_TODO.md` to mark the `clean_text=false` compatibility test item complete.

## 2026-03-22T21:32:32Z - GPT-5.4 - Request-context coverage now asserts selected voice and text length
- Added `synthesize_audio_records_resolved_voice_in_request_context` and `synthesize_audio_records_text_length_in_request_context` in `src/routes/tts.rs` to prove the TTS synthesis path records the resolved voice and input text length in the shared request context.
- The selected-voice test uses alias resolution so the assertion proves the recorded value is the effective backend voice, not just the raw request path token.
- Revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test synthesize_audio_records_`, and `cargo test --all-features`.
- Updated `docs/RUST_PORT_TODO.md` to mark the remaining request-context compatibility test items complete.

## 2026-03-22T21:35:40Z - GPT-5.4 - Phase 8 startup tests now cover boot success and failure paths
- Added startup-path integration coverage in `tests/config.rs` for invalid config rejection, missing `model.onnx`, missing `voices.npz`, and an ignored real-backend success path that initializes app state from a valid config using `KITTENTTS_SERVER_TEST_MODEL_DIR`.
- Kept the deterministic failure-path tests self-contained by generating temporary model directories with only the required missing asset omitted, so the boot path fails at the intended backend verification step.
- Revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features --test config`, and `cargo test --all-features`.
- Updated `docs/RUST_PORT_TODO.md` to mark all remaining Phase 8 startup test items complete.

## 2026-03-22T21:44:29Z - GPT-5.4 - Phase 9 systemd acceptance validated with a live systemd smoke run
- Verified the service can run under systemd by building `target/release/kittentts-server-rs` with `--features real-backend`, launching it as a transient user-managed unit with `systemd-run --user`, and confirming `/healthz` returned `status=ok`, `model_loaded=true`, and `engine=kitten_tts_rs` on port `18081`.
- Journal logs from the smoke run showed the expected startup path under systemd: ONNX Runtime path selected from `ORT_DYLIB_PATH`, `espeak-ng` available, model and voices loaded from the cached Hugging Face snapshot, and the server listening successfully before serving the health request.
- On this Ubuntu host, `systemd-analyze verify --root=...` is not supported for `verify`, so the shipped system unit could not be validated inside a fake install root that way; the acceptance decision was therefore based on the successful live systemd-managed smoke run plus the existing deployment docs/unit wiring.
- Updated `docs/RUST_PORT_TODO.md` to mark `Service can run under systemd` and the Definition-of-done item `systemd deployment works` complete, then revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.

## 2026-03-22T21:50:08Z - GPT-5.4 - Espeak-ng runtime packaging is now explicitly proven and documented
- Rebuilt the final Docker runtime image with `docker build --build-context kitten_tts_rs=../kitten_tts_rs -t kittentts-server-rs:latest .` and verified the packaged container includes a working `espeak-ng` binary via `docker run --rm --entrypoint espeak-ng kittentts-server-rs:latest --version`.
- Combined that container proof with the earlier live systemd smoke run, whose journal showed `espeak-ng` available on the host path before model initialization, to close the remaining deployment packaging gap.
- Updated `README.md` to make the packaging split explicit: Docker bundles `espeak-ng` in the runtime image, while systemd/local deployment expects `espeak-ng` to be installed on the host before startup.
- Updated `docs/RUST_PORT_TODO.md` to mark the Definition-of-done item `espeak-ng is handled correctly in runtime packaging` complete, then revalidated with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.

## 2026-03-22T21:51:33Z - GPT-5.4 - Definition-of-done now matches completed Phase 10.2 compatibility coverage
- Updated `docs/RUST_PORT_TODO.md` to mark the final Definition-of-done item `Compatibility-sensitive behavior is explicitly tested` complete.
- This sync is based on the now-complete Phase 10.2 checklist items covering style-row selection, `clean_text=false`, unknown-voice fallback, request-context selected voice and text length, and OpenAI-shaped auth errors.

## 2026-03-22T21:53:02Z - GPT-5.4 - Phase 10.1 regression checklist synced to explicit Python-vs-Rust comparison
- Compared the Python compatibility target in `KittenTTS_server/app/api.py`, `app/auth.py`, `app/errors.py`, `app/models.py`, and `app/voices.py` against the Rust route/auth/error/voice implementations and tests in `src/routes/*`, `src/middleware/auth.rs`, `src/error.rs`, `src/services/voices.rs`, and `tests/health.rs`.
- Marked the Phase 10.1 items complete for route paths, status codes, response headers, error envelopes, voice listing shape, auth behavior, output-format negotiation, and OpenAI route behavior.
- The comparison confirmed the expected public surface and behavior parity points: `/healthz`, `/v1/voices`, `/v1/text-to-speech`, `/v1/text-to-speech/{voice_id}`, `/v1/text-to-speech/{voice_id}/stream`, and `/v1/audio/speech`; matching auth-policy split for public vs `/v1` routes; local vs OpenAI error-envelope split; ElevenLabs/OpenAI response headers including `X-Output-Format` and `X-Request-Id`; compatible voice descriptor shape and alias metadata; and the same supported WAV/PCM/output-format negotiation rules used by the Python shim.

## 2026-03-22T21:55:02Z - GPT-5.4 - Full Rust validation rerun is clean
- Re-ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` in `KittenTTS_rust_server`.
- Results were clean: unit tests `52 passed, 0 failed, 1 ignored`; config integration tests `13 passed, 0 failed, 1 ignored`; health integration tests `8 passed, 0 failed`; doc tests `0 failed`.
- The ignored tests remain the host-dependent real-backend synthesis and valid-config startup checks that require `KITTENTTS_SERVER_TEST_MODEL_DIR` plus a compatible runtime environment.

## 2026-03-22T21:57:37Z - GPT-5.4 - Deployment parity work checked in on master
- Created commit `165892e` (`feat: complete deployment parity and validation`) with the Rust-server deployment artifacts, compatibility test coverage, README/TODO updates, and the clean validation state.
- Commit `165892e` was created at `2026-03-22T14:57:26-07:00` according to `git log -1 --format="%aI" 165892e`.
- This interaction also included the requested check-in flow to prepare `master` for pushing to GitHub.

## 2026-03-22T22:12:45Z - GPT-5.4 - Phase 10.3 manual side-by-side validation completed
- Ran the Rust server on `127.0.0.1:18081` and the Python compatibility server on `127.0.0.1:18082`, then sent matched requests to `/healthz`, `/v1/voices`, `/v1/text-to-speech`, `/v1/text-to-speech/jasper`, `/v1/text-to-speech/not-a-real-voice`, `/v1/text-to-speech/jasper/stream`, and `/v1/audio/speech` for both `wav` and `pcm`.
- Observed matching success behavior for the compared synthesis routes, matching voice-list JSON shape aside from nested map key order, valid WAV containers from both implementations, and consistent default/explicit/unknown-voice fallback behavior within each implementation.
- Recorded the manual findings in `README.md`, including the accepted health-route metadata differences (`engine`, `engine_version`, and Rust-only ONNX Runtime fields), non-identical waveform bytes and payload sizes across implementations, and the pseudo-stream header difference seen in this environment.

## 2026-03-22T22:14:54Z - GPT-5.4 - Full Rust validation rerun remains clean after Phase 10.3 docs updates
- Re-ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` in `KittenTTS_rust_server` after the manual validation documentation updates.
- Results were still clean: unit tests `52 passed, 0 failed, 1 ignored`; config integration tests `13 passed, 0 failed, 1 ignored`; health integration tests `8 passed, 0 failed`; doc tests `0 failed`.
- The worktree at this point contains the expected documentation-only edits in `README.md`, `docs/RUST_PORT_TODO.md`, and `memory.md`.

## 2026-03-22T22:19:39Z - GPT-5.4 - Phase 10 acceptance is now complete
- Finished the remaining Phase 10.4 cleanup/documentation items by adding compatibility-focused comments in `src/routes/tts.rs`, `src/backend/kitten.rs`, and `src/services/voices.rs`, plus README sections for architecture, model assets/dependencies, pseudo-streaming limits, and non-goals.
- Re-ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`; results remained clean with `52 passed, 0 failed, 1 ignored` in unit tests, `13 passed, 0 failed, 1 ignored` in config integration tests, and `8 passed, 0 failed` in health integration tests.
- Marked Phase 10 acceptance complete in `docs/RUST_PORT_TODO.md` because the port is now documented, test-covered, and the known compatibility differences are explicitly called out in `README.md`.

## 2026-03-22T22:20:42Z - GPT-5.4 - Validation rerun remains clean after Phase 10 acceptance sync
- Re-ran `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` in `KittenTTS_rust_server` after the Phase 10 acceptance checklist update.
- Results were still clean: unit tests `52 passed, 0 failed, 1 ignored`; config integration tests `13 passed, 0 failed, 1 ignored`; health integration tests `8 passed, 0 failed`; doc tests `0 failed`.
- The worktree at this point contains the expected Phase 10.4/10.5 edits in `README.md`, `docs/RUST_PORT_TODO.md`, `memory.md`, `src/backend/kitten.rs`, `src/routes/tts.rs`, and `src/services/voices.rs`.

## 2026-03-22T22:21:42Z - GPT-5.4 - Phase 10 cleanup and acceptance checked in on master
- Created commit `08c567b` (`docs: finish phase 10 cleanup and acceptance`) with the Phase 10.4 README/code-comment updates, the Phase 10.5 checklist completion, and the clean validation state.
- Commit `08c567b` was created at `2026-03-22T15:21:36-07:00` according to `git log -1 --format="%aI" 08c567b`.
- This interaction also included the requested check-in flow to prepare `master` for pushing the completed Phase 10 documentation and acceptance updates to GitHub.
