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
