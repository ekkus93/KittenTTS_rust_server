# KittenTTS Rust Server — Revalidation, Concurrency, and Compatibility Specification

**Date:** 2026-11-18  
**Status:** Proposed; acceptance pending evidence  
**Scope:** Four follow-up workstreams for the existing v0.1 compatibility-focused Rust server  
**Tracking:** [REVALIDATION_COMPATIBILITY_TODO_2026-11-18.md](REVALIDATION_COMPATIBILITY_TODO_2026-11-18.md)  
**Prior art:** [RUST_PORT.md](RUST_PORT.md), [RUST_PORT_TODO.md](RUST_PORT_TODO.md), [CODE_REVIEW_TODO1.md](CODE_REVIEW_TODO1.md), repository `README.md`

## 1. Purpose and baseline

Establish a repeatable, independently verifiable readiness decision for `kittentts-server-rs` after its initial March 2026 implementation. The target is **not** a feature rewrite: prove that it builds and works with the real KittenTTS engine, correct blocking/concurrency behavior where necessary, and obtain enough Python-parity evidence to decide whether current Python clients may migrate.

As of the initial 2026-11-18 review:
- Default branch: `master`; baseline HEAD `fd2d0e40771bfea765976ff9ae22969ecd63700c` (2026-03-28).
- The last `master` GitHub Actions run succeeded; tag `v0.1.0` also passed validation and release jobs. These are **historical** results, not fresh validation.
- `docs/RUST_PORT_TODO.md` still leaves Phase 7.10 (compatibility sufficient to replace the Python server) unchecked.
- March 22 manual tests reported route/format parity, but not bit-for-bit waveform parity.
- The HTTP layer's `synthesize_audio` path in `src/routes/tts.rs` synchronously invokes `Synthesizer::synthesize` from async handlers; the backend serializes access through `Mutex<KittenTTS>`. This suggests a risk of starving Tokio worker threads under concurrent inference, which must be measured and tested—not assumed to be solved.
- The `real-backend` feature depends on the sibling `../kitten_tts_rs` crate. CI checks out `ekkus93/kitten_tts_rs` separately; document exact backend ref alongside each result.

**Baseline is descriptive, not evidence of any task in this spec being complete.**

## 2. Scope and invariant contracts

### In scope
1. Re-run and record reproducible formatting, build, Clippy, unit/integration tests, and release/deployment smoke checks.
2. Exercise real ONNX + `espeak-ng` synthesis and validate actual returned audio.
3. Diagnose and, if demonstrated, remediate blocking request handlers; test correctness, responsiveness, and concurrency.
4. Run the Rust and Python servers against a shared compatibility corpus, resolve regressions, and formally close or retain the Phase 7.10 acceptance item based on evidence.

### Out of scope
- New HTTP endpoints, format codecs, SSML, voice cloning, or remote providers.
- True token/chunk-incremental synthesis; the `/stream` endpoint remains **pseudo-streaming** (generate first, chunk delivery afterward).
- Changing model architecture or rewriting `kitten_tts_rs`.
- Replacing Axum, Tokio, authentication policy, or existing JSON/headers.
- Throughput guarantees unsupported by measured hardware or unbounded parallel model instances.

### Compatibility invariants
- Preserve Python behavior for request validation (strict and permissive), default/alias/case-insensitive voice selection and unknown-voice fallback.
- Preserve public `GET /healthz` and protected `/v1*` behavior, dual API-key semantics, request IDs, and route-specific error envelopes.
- Preserve ElevenLabs-like and OpenAI-like route contracts, WAV and raw signed 16-bit little-endian PCM response semantics, sampling/channel configuration, and `X-Output-Format`.
- Preserve config precedence: built-in defaults → `KITTENTTS_SERVER_*` environment → JSON settings file.
- Preserve `clean_text=false` backend invocation, compatibility-sensitive chunking/style behavior, startup fail-fast on absent required dependencies, and the separate sibling backend boundary.
- Avoid logging raw TTS input, API keys, or other secrets in CI artifacts.

## 3. Workstream A — Fresh builds and CI validation

### Design
Use the existing GitHub Actions `CI` workflow and run tests at a specific, recorded **exact commit SHA**. Do not infer that today's source is healthy from March's badges. First inspect actual workflow, Rust toolchain, `Cargo.lock`, backend checkout, release instructions, and failures. Record server commit, backend commit, toolchain/compiler, OS, ONNX Runtime version, `espeak-ng` version, test commands, workflow/run links, and artifact provenance.

Required validation commands from the server checkout with `../kitten_tts_rs` available:

```sh
cargo fmt --check
cargo check --features real-backend
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --features real-backend
```

Include an intentionally model-free default-feature validation where useful, but do **not** present this as evidence of real synthesis. Investigate failures and distinguish source regression, stale pinned/dynamic dependencies, missing system libraries, backend version drift, and CI infrastructure. Prefer minimal fixes with separate test evidence.

The CI workflow currently checks out the backend branch without an explicit commit pin. For reproducibility, record and consider pinning that dependency to a reviewed exact backend SHA (with a documented update process). **Do not change CI policy, tools, dependency versions, or new dependencies without required approval under `.github/copilot-instructions.md`.**

### Acceptance
- A **fresh** pass is recorded for the current exact `master` HEAD, not merely a previous tag or older SHA; all mandatory checks succeed.
- The server and backend SHAs, commands, and toolchain/runtime environment are recorded in a durable evidence note or test report.
- The release binary build passes; deployment smoke evidence identifies whether it tested binary, Docker, and/or systemd. Untested deployment methods remain explicitly unverified.
- A failed or missing check is never interpreted as success.

## 4. Workstream B — Actual end-to-end ONNX speech tests

### Design
Use `--features real-backend`, `espeak-ng`, a working ONNX Runtime shared library, and the real KittenTTS model (`KittenML/kitten-tts-nano-0.8` or a documented, equivalent local asset set). The test harness must **start the server process**, wait for readiness, issue HTTP requests, and validate real synthesized audio. Fake/fixture synthesizers remain valuable for fast tests, but cannot satisfy this requirement.

Make the real-model workflow an explicit opt-in validation (manual GitHub Actions dispatch or a documented external/local runner) when model download/caching cost, license, network availability, or runtime makes it unsuitable for ordinary PR CI. It must fail clearly when prerequisites are missing rather than silently skip and claim a pass. Cache model files only where permitted; do not upload model weights or large speech recordings as build artifacts by default.

Minimum matrix:
- `/healthz`: 200, loaded model, expected metadata.
- `/v1/voices`: nonempty inventory and expected voice selection/alias behavior.
- `/v1/text-to-speech` default voice and `/{voice_id}` known voice: valid, nontrivial WAV.
- `/{voice_id}/stream`: complete valid WAV/PCM body with expected pseudo-stream semantics and headers.
- `/v1/audio/speech`: WAV and raw PCM; test `model: "tts-1"`.
- At least one negative case each for missing model/voice assets or `espeak-ng`, malformed request, and required auth (negative cases may use isolated launch configurations).
- Use a small deterministic set of short prompts; record exact asset versions, selected voice, and requests.

For WAV verify RIFF/WAVE header, decoding, 24 kHz mono at defaults (or expected configured rate/layout), sample count > 0, finite/non-silent output under a documented threshold, and reasonable nonzero duration. For PCM verify S16LE bytes, expected even byte count, nonzero samples, and consistent decoded sample rate/channel interpretation. Avoid exact waveform hash comparisons: runtime/platform numerical differences may be legitimate. Detect gross failures (empty/silent/truncated/corrupt output). Optionally collect listening notes or perceptual metrics, clearly labeled as subjective or approximate.

### Acceptance
- A fresh run with the actual model successfully produces and parses both WAV and PCM.
- Every required real-backend route is exercised; tests prove more than a mocked HTTP handler.
- Fail-fast dependency cases and response/error constraints are tested.
- Exact environment, logs, test invocation, and results are preserved with failures identified.

## 5. Workstream C — Async blocking, contention, and concurrency

### Problem to measure
Current `src/routes/tts.rs` handlers are `async`, but call synchronous `synthesize_audio`, which invokes `Synthesizer::synthesize` directly. `KittenBackend` holds a `Mutex<KittenTTS>` while generating. A long inference on a Tokio worker can block unrelated requests, even if the model is deliberately single-flight. Parallel HTTP traffic must not cause corruption, deadlocks, starvation, or incorrect request metadata.

### Implementation boundaries
- Establish **before** measurements and a deterministic test synthesizer that blocks on a controlled gate to prove scheduler responsiveness without downloading model weights.
- If blocking is confirmed (or structurally established with a reproducible test), move CPU/blocking synthesis off Tokio worker threads—prefer `tokio::task::spawn_blocking` around the synchronous generation path. Keep request validation and HTTP response/error shaping behavior intact. Avoid holding a `MutexGuard` across `.await`.
- Map `JoinError` to an appropriate existing app-level error while preserving the OpenAI-style vs local envelope split and `X-Request-Id`. Panics must not silently turn into successful empty audio.
- Keep the single model instance and its lock unless measurements support a more involved change; `spawn_blocking` makes the scheduler responsive but **does not increase single-model synthesis parallelism**.
- Evaluate bounded admission/backpressure if measured queue growth or memory pressure warrants it; do not invent silent hard limits or change API behavior without documenting compatibility implications.
- Test simultaneous requests with different voices and text, a long-running synthesis plus a `/healthz` request, auth/invalid input while synthesis is pending, and post-failure recovery. Ensure cancellation/disconnected clients do not corrupt the backend or leak request state. Document any unavoidable completion of in-flight blocking work after client disconnect.

### Acceptance
- A controlled regression test demonstrates that `/healthz` and unrelated lightweight requests remain responsive while synthesis is in flight; use generous bounded timing with explicit CI environment tolerance and deterministic synchronization rather than sleeps alone.
- Multi-request tests return correct status, media types, nonempty valid audio, request-specific IDs/voice metadata; no deadlocks or cross-request leakage.
- A real-model concurrency smoke run measures wall time, throughput, p50/p95 latency, and memory under a stated low/moderate load profile, with no universal speedup claim.
- Existing integration tests and strict Clippy checks still pass; architecture change is minimal and preserves all Phase 7 contracts.

## 6. Workstream D — Python compatibility and replacement sign-off

### Golden comparison harness
Construct a source-controlled **request corpus and comparator** (not fixed golden waveform bytes) against the existing Python `KittenTTS_server` and this Rust server. Use the same model assets, voice settings, sample rate, environment and API keys where possible. Run both services on separate localhost ports; capture system/package/model revision and configuration. Use fresh HTTP requests rather than old README examples as sole evidence.

Cover at least:
1. `GET /healthz` and `GET /v1/voices` (canonicalize JSON object key order).
2. ElevenLabs default, explicit known/alias, unknown/fallback voice.
3. Pseudo-stream response: full-body equivalence of container shape and declared format, with chunked transfer distinctions tracked separately.
4. OpenAI `/v1/audio/speech` WAV and PCM, valid model identifier `tts-1`.
5. Request validation: empty/malformed text, unsupported output formats, strict/permissive mode, invalid model as specified by baseline, and HTTP method/path edge cases.
6. Auth: disabled, valid `xi-api-key`, valid bearer, missing/wrong credential, conflicting dual headers.
7. Config: explicit JSON-over-env precedence, default voice/map, sample rate, mono/stereo.
8. Long/multi-chunk text, speed changes, voice differences and audio health.

Compare status code, normalized JSON shape/values, relevant headers, response formats, WAV metadata, PCM semantics, failure envelopes, and voice selection. Allow documented nonsemantic differences (request IDs, timing, ordering of JSON map keys). **Do not silently accept differences** in behavior or payloads: classify each as defect, harmless platform/metadata variance, or an explicitly approved waiver. Exact waveform identity is not mandatory, but audio must pass format/quality sanity checks and voice behavior.

### Replacement gate
- Produce a dated compatibility matrix with each case marked PASS / FAIL / WAIVED / BLOCKED; provide reproduction and logs for failures.
- Fix regressions; rerun the affected suite and fresh exact-head CI.
- Require explicit human acceptance of any behavioral deviation material to existing clients. Clearly document observed health metadata/ONNX diagnostic fields and pseudo-streaming transfer differences.
- Only check the existing `docs/RUST_PORT_TODO.md` Phase 7.10 acceptance item after **all** mandatory cases pass or approved waivers are documented and replacement readiness is affirmed. Do not mark it complete for a passing build alone.

## 7. Work order, evidence and handoff

Recommended sequence: **A baseline CI → B real synthesis → C concurrency diagnosis/remediation → D compatibility sign-off → A final exact-head CI rerun**. Independent deterministic tests in C can proceed before model-backed B is available, but real-model work is not considered complete until actually run.

Use one evidence ledger within `docs/` or a dedicated durable Markdown report, linked from the TODO, that records:
- Timestamp and exact server/backend commits (plus the Python reference revision for D).
- Commands/workflow IDs/run links and environment versions.
- Test counts, route matrix, performance measurement configuration, pass/fail/blocker.
- Defects, rationale, remaining risks, waivers, and the precise reason for any blocked task.
- Changed paths and evidence for closing each checklist item.

Any user-dependent action (manual listening, production traffic cutover, access to unavailable model assets, approval of CI/dependency-policy changes) should be isolated as a **blocker only for that task**, while independently actionable engineering tasks continue.

## 8. Definition of done

This follow-up is complete only when:
1. Current exact-head validation and release build are green, with reproducibility metadata.
2. The actual ONNX model is proved to generate valid WAV and PCM via the live HTTP server.
3. Nonblocking handler behavior and concurrency/cancellation safety are tested, with measured real-model results.
4. The Python parity matrix is complete, material differences are resolved or explicitly approved, and the legacy Phase 7.10 acceptance is justified.
5. TODO checkboxes are updated **only with links to evidence**; unknown or blocked outcomes remain unchecked.

No workstream may be declared done from documentation alone.
