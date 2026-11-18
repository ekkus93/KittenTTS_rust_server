# KittenTTS Rust Server — Revalidation Evidence Ledger

**Opened:** 2026-11-18  
**Scope:** [Revalidation / compatibility specification](REVALIDATION_COMPATIBILITY_SPEC_2026-11-18.md) and [implementation TODO](REVALIDATION_COMPATIBILITY_TODO_2026-11-18.md)

Evidence refers to **specific source commits and runs**, not a blanket statement that all follow-up acceptance is complete. This record is the durable handoff for subsequent Ralph Loop iterations.

## 2026-11-18 — Baseline, regression recovery, and initial concurrency work

### Provenance (A1.1, A1.2, A1.3)
- Server repository: `ekkus93/KittenTTS_rust_server`; default branch `master`. Before this loop: `19a5e5fc18b465c9e8e8a00aa31313d3d4d6de1b` (new spec/TODO); prior March source baseline `fd2d0e40771bfea765976ff9ae22969ecd63700c`. Current code validation at `276120cf80dd351c8545ca10e9af03f450921bee`.
- Backend repository: `ekkus93/kitten_tts_rs`, branch `main`, observed exact checkout `df7718161bc68c8b13bea85e879308becf12e87b` in November CI job logs (not inferred). Ralph Bridge direct backend repository access was unavailable, but the **server's workflow log confirms** its backend checkout and SHA.
- Environment: GitHub-hosted `ubuntu-latest` resolved to Ubuntu **26.04**, x86_64; installed Rust stable **1.100.0** (2026-11-12 channel release, version from run logs). `espeak-ng` package version visible in the runner log: `1.52.0+dfsg-5build1`; `Cargo.lock` uses v4 lockfile.
- Manifest and packaging reviewed: `Cargo.toml` edition 2021, local optional path dependency `../kitten_tts_rs`, Axum 0.8 and Tokio; `.github/workflows/ci.yml`, `Dockerfile`, `README.md` runtime requirements. ONNX Runtime is dynamically loaded and Docker packaging specifies **1.24.2**; the current CI workflow does not perform inference or ship the shared library.
- Reproducibility gaps: GitHub Actions checks out the backend default branch without a revision pin, uses a moving stable Rust toolchain and moving `ubuntu-latest`; Docker builder specifies Rust **1.88**, unlike current CI; default Hugging Face model assets are not pinned to an immutable revision by this server. Pinning CI/backend requires the repository's ask-first approval; no CI policy was changed.

### CI evidence (A2.1, A2.3, A2.4, A2.7)
- [Run #10](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44404639251) at `19a5e5f`: **failed** Clippy due to Rust 1.100 `chunks_exact_to_as_chunks` lint in three audio conversion locations. Formatting passed; tests skipped.
- Commit `6e3ab26c46e1bf0a0738b4c1a5a5e69b16273bfa` replaced those three loops with the equivalent `as_chunks::<2>()` forms, without lint suppression or changed audio semantics. [Run #11](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44405900950): **passed** formatting, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.
- The first concurrency implementation at `23f7b25` exposed formatting requirements; follow-up `daf9677` addressed rustfmt. At `7103043`, Clippy exposed an ownership error in the OpenAI error path; commit `276120cf80dd351c8545ca10e9af03f450921bee` preserved the request context using an `Arc` clone.
- [Run #15](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44407996793) at `276120c`: **passed**. Validated steps include `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features`. Tests: **92 library + 5 binary + 13 configuration integration + 20 HTTP integration = 130 passing, 0 failed, 0 ignored**. This proves compilation and test success with the real-backend **feature enabled**, but the tests do not invoke full ONNX inference.
- Outstanding command-specific evidence: the workflow does **not** run standalone `cargo check --features real-backend` or `cargo build --release --features real-backend` on a normal branch push. A successful Clippy typecheck is not a recorded execution of the former command. Release build is only historically documented for the March `v0.1.0` tag. A fresh release build is pending.
- An intermediate `daf9677` CI run (#13) was still observed running at the time of review despite later successful run #15; no acceptance depends on that older run.

### Real-model smoke harness (B1.1–B1.4)
- Commit `7103043a3774cb22230063c1474f043494f78f24` adds opt-in `tests/live_backend_smoke.py` (Python 3 standard library; no dependency changes).
- Build: `cargo build --release --features real-backend` with sibling backend checkout. Execute: `python3 tests/live_backend_smoke.py`. Provide ONNX Runtime through `ORT_DYLIB_PATH` or supported discovery, `espeak-ng`, and model assets via `KITTENTTS_SERVER_MODEL_DIR` or the default Hugging Face cache.
- The script starts the actual binary on an ephemeral localhost port, waits for `model_loaded=true`, checks voices, generates default/named/fallback WAV, pseudo-stream WAV/PCM, and OpenAI WAV/PCM; validates RIFF/PCM data, rate/channels, duration, sample alignment, and non-silent signal. It checks empty text / invalid OpenAI model rejection and always shuts down the spawned server.
- **Not yet executed with a real ONNX model; B2 remains open.** CI currently runs only Rust unit/integration tests, which use fake synthesis.

### Concurrency (C1.1, C2.1, C2.3, C3.5)
- In `src/routes/tts.rs`, asynchronous Axum handlers previously synchronously performed `Synthesizer::synthesize`; the backend's `Mutex<KittenTTS>` locked around inference. This could block a Tokio HTTP worker; serial backend behavior is intentional for a single model.
- Commit `23f7b25` (with formatting and context fixes through `276120c`) uses `tokio::task::spawn_blocking` for synchronous speech generation/normalization while maintaining HTTP-visible route behavior. The blocking closure owns its data, and no backend lock crosses `.await`. The OpenAI error path retains its error-context Arc.
- New deterministic regression test: with a single Tokio worker and a synthesizer blocked on a `Condvar`, `GET /healthz` must complete before synthesis is released; a separate test checks blocking task panic maps to an application error. Both are included in the 92 passing library tests on CI #15.
- A detached/abandoned `spawn_blocking` task may continue computation after client disconnect. This is documented in source, but **cancellation and real-model load measurements have not yet been verified**. Do not claim greater concurrent single-model inference throughput.
- Before/after benchmark, concurrent voice/text isolation, error-envelope regression matrix, cancellation test, and measured real-model p50/p95/memory remain open.

### Deployment variants (A3.5)
| Path | Latest status | Reason |
| --- | --- | --- |
| Source CI tests | PASS on run #15 | Rust unit/integration coverage; no inference |
| Release compilation | UNVERIFIED fresh | Tag `v0.1.0` built in March, but no November `cargo build --release` |
| Linux systemd runtime | UNVERIFIED fresh | No host installation in this run |
| Docker build and runtime | UNVERIFIED fresh | Existing Dockerfile/compose reviewed, not executed |
| Real model smoke | BLOCKED / NOT RUN | No approved real-model runner executed `tests/live_backend_smoke.py` yet |

### Follow-up / approvals
- **Approval needed for CI-policy edits:** pin Rust/backend SHAs and optionally add opt-in model-backed validation/release-build jobs. Repository `.github/copilot-instructions.md` requires approval for CI policy changes.
- **Real inference test environment needed:** model assets, `espeak-ng`, ONNX Runtime (shared library), appropriate CPU/RAM/network/cache access, and a runner able to run the opt-in harness. Until a real-model test actually runs, B2 and the final readiness gate remain open.
- **Python parity environment needed:** exact Python reference revision and separately running Python/Rust servers configured alike. March side-by-side tests are historical and not enough to close Phase 7.10.
- Continue independently actionable tests/docs. Do not change `docs/RUST_PORT_TODO.md` Phase 7.10 without measured parity evidence.
