# KittenTTS Rust Server — Revalidation and Compatibility TODO

**Created:** 2026-11-18  
**Status:** In progress; completed tasks cite `docs/REVALIDATION_EVIDENCE.md`  
**Specification:** [REVALIDATION_COMPATIBILITY_SPEC_2026-11-18.md](REVALIDATION_COMPATIBILITY_SPEC_2026-11-18.md)  
**Original port tracker:** [RUST_PORT_TODO.md](RUST_PORT_TODO.md) (Phase 7.10 remains unchecked)  
**Working branch:** `master`; perform exact-head reads and compare-and-swap commits; no task branches or PRs unless explicitly requested.

> All checkboxes below begin unchecked. Existing March 2026 tests and release artifacts are historical baseline, **not** acceptance evidence for November revalidation. Never mark a task done merely because CI was green eight months ago.

**Evidence ledger:** [REVALIDATION_EVIDENCE.md](REVALIDATION_EVIDENCE.md) (exact SHAs, CI, blockers, and partial validation)

## Operating rules for each implementation pass

- Read current `master` SHA, entire applicable TODO/spec, repository policies, changed code, and exact-head CI before starting.
- Select the highest-priority independently actionable unchecked task; implement actual code/tests where required instead of producing analysis-only checkmarks.
- Use the separate `../kitten_tts_rs` backend dependency; capture its exact commit. Do not modify dependencies, CI policy, or architecture without the repository's required approval.
- Test and commit each logical increment; after every source change, verify the new exact-head CI result. Preserve incomplete work and never invent success.
- Record results, exact SHAs, links, and blockers in a dated evidence note (proposed: `docs/REVALIDATION_EVIDENCE.md`). Creation of that note is part of implementation, **not** an already-existing deliverable.
- If a model-asset/download/human-approval step is blocked, record why and continue independent deterministic work.
- Keep compatibility-first v1 semantics, including pseudo-streaming, auth/error shapes, config precedence, voice behavior, `clean_text=false`, and existing routes.

## A. Refresh build/test/CI evidence — P0

### A1. Inventory and baseline
- [x] **A1.1** Capture exact current server `master` SHA, backend `kitten_tts_rs` SHA, default branch/tag state, and current CI runs.
- [x] **A1.2** Inspect `Cargo.toml`, `Cargo.lock`, workflow, `Dockerfile`, documented ONNX Runtime/`espeak-ng` requirements; enumerate reproducibility gaps.
- [x] **A1.3** Start an evidence ledger with baseline toolchain, OS, backend revision, environment, commands, and CI links.

### A2. Run validation
- [x] **A2.1** Verify `cargo fmt --check`.
- [ ] **A2.2** Verify `cargo check --features real-backend`.
- [x] **A2.3** Verify `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] **A2.4** Verify `cargo test --all-features` and record counts/failures; check no test was unintentionally skipped.
- [ ] **A2.5** Verify `cargo build --release --features real-backend` with the actual sibling checkout.
- [x] **A2.6** Trigger or observe a *fresh* GitHub Actions validation for the exact current HEAD; record result, run ID, status, and date.
- [x] **A2.7** Diagnose and fix any reproducible failure; rerun the exact impacted checks and CI.

### A3. Release/deployment reproducibility
- [ ] **A3.1** Review backend checkout nondeterminism and propose a pinned backend-revision strategy; seek required CI-policy approval **before** any workflow changes.
- [ ] **A3.2** Smoke-test a fresh release binary on Linux with required runtime dependencies, or record the exact environment blocker.
- [ ] **A3.3** Smoke-test container startup, `/healthz`, real-backend availability, and shutdown if CI resources allow; do not mislabel Docker build-only as runtime coverage.
- [ ] **A3.4** Validate documented systemd installation in an appropriate host/VM, or mark as **not yet verified** with a reproducible procedure.
- [x] **A3.5** Record pass/fail/blocker per deployment variant in evidence ledger.

**A acceptance:** exact-current-HEAD validation is green with a reproducible server/backend/environment provenance; release build and deployment outcomes are reported without overstating coverage.

## B. Real ONNX synthesis and audio checks — P0

### B1. Harness and prerequisites
- [x] **B1.1** Design a repeatable opt-in live-server test harness using `--features real-backend`, `espeak-ng`, ONNX Runtime, and real model assets; document cache/local-path setup and license/network limits.
- [x] **B1.2** Create minimal short-text test fixtures for default voice, known voice/alias, unknown fallback, stream, and OpenAI WAV/PCM.
- [x] **B1.3** Make startup readiness, bounded test timeouts, teardown, diagnostics, and explicit prerequisites deterministic; no silent success/skip for missing model assets.
- [x] **B1.4** Add WAV/PCM validators: container/header correctness, sample rate/channels, nontrivial duration/sample count, finite/non-silent signal, and PCM byte alignment.

### B2. Exercise actual engine
- [ ] **B2.1** Start the full model-backed Rust server; confirm `/healthz` and `/v1/voices`.
- [ ] **B2.2** Generate and verify WAV for default, explicitly named, and fallback voice paths.
- [ ] **B2.3** Generate and verify full pseudo-stream body and response headers (WAV; supported PCM variant as applicable).
- [ ] **B2.4** Generate and verify OpenAI-compatible `/v1/audio/speech` WAV **and** PCM using `model: "tts-1"`.
- [ ] **B2.5** Check malformed input, protected route auth, and fail-fast missing model/`voices.npz`/`espeak-ng` behavior; isolate configurations as necessary.
- [ ] **B2.6** Run the live-engine scenario on a documented environment (opt-in GitHub Actions or local runner); capture actual runner, model/ORT revisions, results, and failure logs.
- [ ] **B2.7** Address observed real-engine failures and revalidate.

**B acceptance:** real model inference has produced verified nonempty, non-silent, correctly formatted audio on all required routes; mock-only tests do not count.

## C. Synthesis concurrency and async responsiveness — P1

### C1. Reproduce the risk
- [x] **C1.1** Trace sync call path in `src/routes/tts.rs` and `src/services/synth.rs`; identify where inference blocks Tokio and where backend `Mutex<KittenTTS>` is held.
- [ ] **C1.2** Add a deterministic blocking fake synthesizer/gate test: one in-flight synthesis and an independently responsive `/healthz`; prove the test catches the pre-fix behavior.
- [ ] **C1.3** Record before measurements for 1, 2, and a modest simultaneous-request count; note CI worker constraints.

### C2. Minimal safe remediation
- [x] **C2.1** Move synchronous TTS work to an appropriate `spawn_blocking` boundary; preserve request validation, response shape and voice resolution.
- [x] **C2.2** Preserve local and OpenAI error-envelope distinctions and request IDs on backend, panic/join, validation, and authorization failures.
- [x] **C2.3** Keep a single backend instance with safe mutex ownership unless measurements justify a larger change; explicitly verify no lock is held over an `.await`.
- [x] **C2.4** Document what happens if an HTTP client disconnects during blocking inference; prevent corruption, race conditions, and accidental cross-request state.
- [ ] **C2.5** If bounded admission/backpressure is necessary, document its tradeoffs and obtain approval for any outward-facing behavior change.

### C3. Verify concurrency
- [x] **C3.1** Test concurrent valid requests with distinct voices/text and validate audio, headers, IDs, and selected voice isolation.
- [x] **C3.2** Test `/healthz`, invalid input, and authentication while synthesis is in flight; no scheduler starvation/deadlock.
- [x] **C3.3** Test backend failure/panic recovery and client cancellation where feasible.
- [ ] **C3.4** Run real-model low/moderate concurrency benchmark: wall time, throughput, p50/p95 latency, peak memory, and hardware/runner description.
- [x] **C3.5** Confirm format, Clippy, existing test suite, and exact-head CI still pass after code changes.

**C acceptance:** deterministic non-starvation tests pass and measured real-model concurrent requests are safe; do not mistake offloading for parallel model execution.

## D. Python compatibility and replacement acceptance — P1

### D1. Define apples-to-apples parity
- [ ] **D1.1** Capture exact Python `KittenTTS_server` revision and equivalent Rust/backend/model/voice config.
- [x] **D1.2** Build a repeatable request corpus and comparator with explicit normalizations (JSON key order, transient request ID/timestamps).
- [x] **D1.3** Define explicit PASS / FAIL / WAIVED / BLOCKED classification and artifact/evidence format before testing.

### D2. Execute route matrix
- [ ] **D2.1** Compare `/healthz` metadata and `/v1/voices` inventory/descriptor fields.
- [ ] **D2.2** Compare ElevenLabs-style default, named, alias, unknown/fallback voices.
- [ ] **D2.3** Compare stream status, headers, negotiated WAV/PCM formats and decoded body, respecting pseudo-stream semantics.
- [ ] **D2.4** Compare OpenAI WAV and PCM success, invalid model/input and OpenAI-style error envelopes.
- [ ] **D2.5** Compare missing/valid/invalid/conflicting authentication and `X-Request-Id` semantics.
- [ ] **D2.6** Compare strict vs permissive validation, unsupported formats, JSON-vs-env precedence, default and custom sample rate/channel configuration.
- [ ] **D2.7** Compare multi-chunk/long text, speed changes, voice behavior, and audio validity/quality (not exact waveform hashes).
- [ ] **D2.8** Publish dated case-by-case matrix and record every material difference with reproduction and classification.

### D3. Final decision
- [ ] **D3.1** Fix material compatibility regressions and rerun relevant tests; explicitly request approval for any remaining material waiver.
- [ ] **D3.2** Run final current-master exact-SHA CI and refreshed real-model smoke suite after changes.
- [ ] **D3.3** State explicit decision: READY / NOT READY / READY WITH APPROVED WAIVERS, including remaining deployment/operational risks.
- [ ] **D3.4** Only on supported READY sign-off, mark the outstanding **Phase 7.10** criterion in `docs/RUST_PORT_TODO.md` complete and link the evidence; otherwise leave unchecked.
- [ ] **D3.5** Update README compatibility/deployment notes to match verified behavior and any approved limitations.

**D acceptance:** reproducible parity evidence exists, no unapproved client-visible regressions remain, and the legacy acceptance checkbox is updated honestly.

## Final gate

- [ ] **G1** Fresh exact-master CI + release build verified.
- [ ] **G2** Real ONNX speech generation validated end to end for WAV and PCM.
- [ ] **G3** No Tokio worker starvation; concurrency/cancellation tests and measurements recorded.
- [ ] **G4** Python-client compatibility matrix complete; differences resolved or explicitly approved.
- [ ] **G5** Final replacement readiness and Phase 7.10 status match the evidence, with open blockers clearly identified.

**Completion rule:** finish independent actionable software tasks even if a hardware/model/network/manual step is blocked; do not close a checkbox without concrete evidence.
