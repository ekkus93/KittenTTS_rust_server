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

## 2026-11-18 — Additional deterministic tests and parity/benchmark tooling

### Follow-up source commits and Rust CI
- `0b96801a00b9b2610067df08bb389029ebc1194e`: add `tests/compare_python_rust.py`, a Python standard-library side-by-side HTTP comparison harness. It normalizes JSON key ordering and transient request IDs, checks WAV/PCM structural validity, records differences in duration/stream transfer encoding/engine metadata, and writes per-case **PASS/FAIL/BLOCKED** evidence to JSON. **Not executed against two live services.** This closes harness design (D1.2–D1.3), not D2.
- `22e4a6f` / `f9693e1`: add and rustfmt tests for concurrent voice-specific WAV results and distinct request IDs, plus local/OpenAI error-envelope behavior when the blocking synthesis task panics. These tests later passed in CI.
- `9a4d98c390946662efef33deb10f676b155e5d0a`: add deterministic HTTP cancellation test using a fake serialized backend. A request is aborted while the first synthesis is gated; after release the next request must still complete. [Run #20](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44410107649): **passed**.
- `ebfcb4a4534f2e37e7bd5fc5505713cfb7e0e36f`: add `tests/benchmark_real_tts.py` for opt-in, **real-model** 1/2/4 client load with wall time, throughput, p50, nearest-rank p95, and optional Linux RSS observations; [run #21](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44410381777): **passed Rust CI**. **Actual performance measurements have NOT been executed.**
- `2a95070` / `8a47f5d0bb17b72db81afc36cd7267b82f342de2`: extend the single-worker gated test to prove invalid input and unauthenticated requests also complete while a valid TTS request is blocked, with strict rustfmt cleanup. [Run #24](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44411375833): **passed**. Test counts: **95 lib + 5 bin + 13 config + 20 HTTP integration = 133 passed, 0 failed, 0 ignored**.
- `751316184e9248c4258f119b969c0b3b00d33adc`: document opt-in smoke, parity, and benchmark commands in README. The temporary failures on adjacent commits were rustfmt-only (the expanded assertion was too long); current source is formatted and verified.
- `70d595feae4e8338a909c7d05865fcb91dc5e5e7`: extend opt-in real smoke script with configured voice alias (`Narrator` → `Jasper`) and separate fail-fast startup checks for missing model config, missing `voices.npz`, and absent `espeak-ng`. [Run #25](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44411754423): **passed Rust CI**, but this workflow does **not** execute Python scripts or real-model inference. B2 remains **not run**.
- See the [CI runs](https://github.com/ekkus93/KittenTTS_rust_server/actions) for exact-run status; do not extrapolate a model-backed pass from Rust tests.

### What the new tests actually establish
- C2.2: two distinct HTTP error-envelope styles still surface backend panic as an error, with request IDs, rather than silently returning audio.
- C2.4/C3.3: deterministic simulated cancellation does not poison a single-instance/serialized fake backend; the underlying real ONNX model has not been stress-tested.
- C3.1: concurrent requests carry independent request IDs and voice-specific WAV sample values.
- C3.2: health, bad-input validation, and auth checks remain responsive under a deliberately gated synthesis call on a one-worker Tokio runtime.
- A2.6: the November `master` code revisions have passing fresh exact-SHA CI, including [run #25](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44411754423). Docs-only follow-up commits create new runs that must be inspected independently.

### Historical blockers (updated by the later verified CI section)
1. **Required workflow approval:** `.github/copilot-instructions.md` requires the user's express approval before changing CI policy. Proposed change: add a manual/opt-in job for `cargo check --features real-backend`, release build, real ONNX smoke harness and optional benchmark; record a pinned/verified backend revision and controlled Rust toolchain instead of moving `stable`. This was **not** implemented.
2. **Real model runner:** source repo CI currently does not invoke inference; obtain execution on an appropriate runner with compatible ONNX Runtime shared library, `espeak-ng`, model weights/cache, and CPU/RAM/network capacity. Passing Rust CI is not enough for B2, C3.4, A3.2–A3.4.
3. **Python comparison access:** identify the exact Python `KittenTTS_server` ref and provide a way to run it alongside Rust. Ralph Bridge currently cannot read the separate Python/backend repos directly, although the server CI successfully checked out the backend SHA recorded above. No D2 suite was executed; existing `docs/RUST_PORT_TODO.md` Phase 7.10 remains unchecked.
4. **Runtime deployment:** no fresh Docker/systemd installation was performed. These are unverified, **not failed**.
5. **Python scripts themselves:** the opt-in Python utilities are committed but have not been executed or syntax-checked in the existing Rust-only CI workflow. They must be validated in the designated real-model/parity workflow when approved.

### Next actionable pass
- With approval, extend the GitHub Actions workflow for an opt-in, bounded and reproducible real-backend job; run it against a recorded server/backend/model revision and fix observed issues.
- With Python reference availability, run comparison cases and record actual differences/waivers. Keep the original Phase 7.10 unchecked until the replacement decision is evidence-backed.
- After any change, refresh exact-head CI, update this ledger with observed results, and only check TODO tasks that have concrete acceptance evidence.


## 2026-11-18 — Explicit CI approval, release rebuild, and **real-model verification**

### Approval, reproducibility and CI implementation
- User explicitly authorized edits to `.github/workflows/ci.yml` on 2026-11-18.
- Current pinned validation configuration: Ubuntu **26.04** x86_64, Rust **1.100.0**, sibling backend **`df7718161bc68c8b13bea85e879308becf12e87b`**, and ORT **1.24.2** from a versioned PyPI binary wheel. Rust CI keeps `cargo fmt --check`, `cargo check --features real-backend`, strict Clippy, `cargo test --all-features`, and Python syntax checks.
- Opt-in workflow uses `workflow_dispatch` or a `master` commit explicitly containing `[run-real-model]`; adding `[run-benchmark]` enables the additional workload. Normal push/PR commits remain model-free. Separate optimized `cargo build --release --locked --features real-backend` runs before actual HTTP tests; artifact is reused without rebuilding.
- Model repository: `KittenML/kitten-tts-nano-0.8-fp32`; model revision explicitly pinned to **`e73cfb095bd949c786ccdcf3b8ab04eb36c47442`**. GitHub model/ORT SHA provenance and the test/benchmark report are attached to exact-head runs. Model weights and synthesized audio are **not** uploaded.

### Resolved operational failures
- [Run #28](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44416884598) contained a workflow-expression error: `runner.temp` referenced at job scope, before a runner existed. Corrected by initializing runtime paths in a step.
- [Run #29](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44417014152) proved the pinned feature build and Python syntax checks. [Runs #30](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44417267045) and [#31](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44417801705) each verified the optimized release build, but real-model installation failed because the GitHub ONNX Runtime 1.24.2 archive URL returned **HTTP 404**.
- Replaced the unavailable release asset with the `onnxruntime==1.24.2` binary wheel from PyPI, extracting the native library and companion providers. No Cargo dependency changes; record the wheel/library SHA-256 in the CI evidence artifact. The CI input uses `actions/setup-python` and explicitly avoids relying on a system-wide ONNX Runtime installation.

### Successful real synthesis acceptance (A2.2, A2.5, A3.1–A3.2, B2.1–B2.6, G1–G2)
- [Run #33](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44419479645) on `b1f20a4c5534f3aee7f138bd3efde9d890d4d571`: **PASS** validation + optimized release + actual ONNX smoke. Verified `/healthz`, `/v1/voices`, default, named, alias and fallback voices; full pseudo-stream WAV/PCM; OpenAI WAV/PCM; empty/invalid input; missing model config, missing `voices.npz`, missing `espeak-ng`. All eight synthesis examples generated 64,000 validated non-silent audio samples each.
- [Run #34](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44419962792) on **`b2df1cd1b1ecef502e24bc67da761ac88c57fff9`**: **PASS** all three jobs, including authenticated real-backend startup and protected `/v1/voices` requests (missing, matching xi-api-key, Bearer, conflicting keys) and OpenAI missing-auth envelope. Rust validation: **95 library + 5 binary + 13 config + 20 HTTP integration = 133 tests, all passing**. The optimized binary was compiled with `--locked --features real-backend`.
- The low/moderate real-model benchmark in run #34 is actual speech generation through HTTP, not a synthetic inference mock. All **24 requests** succeeded.

| Concurrent clients | Requests | Wall time | Median (p50) | Nearest-rank p95 |
| --- | ---: | ---: | ---: | ---: |
| 1 | 8/8 | 1.42 s | 0.1759 s | 0.1931 s |
| 2 | 8/8 | 1.41 s | 0.3492 s | 0.3832 s |
| 4 | 8/8 | 1.61 s | 0.8039 s | 0.9266 s |

- Peak observed Linux RSS, exact runner/platform data and per-level failed-request diagnostics are written by `tests/benchmark_real_tts.py` to `real-tts-benchmark.json` uploaded as part of `real-model-evidence-<SHA>` in the run #34 artifacts. The numeric RSS value was **not read from the artifact** during this Ralph Bridge review, so do not infer one from the console.
- Latency increases with client concurrency because one `KittenTTS` backend instance remains serialized behind a mutex. These results establish **correctness for a modest tested load**, not high-scale production capacity or parallel model execution.
- Run #34 is the **source-code verification SHA** for the latest CI, Rust, and Python script logic. Subsequent documentation-only commits do not change the compiled binary but must have their own CI checked before reporting exact-head status.

### Outstanding scope and user handoff
- **Python reference:** Ralph Bridge currently has no direct read access to the canonical Python `KittenTTS_server` repository. Obtain the exact repo/ref and equivalent Python runtime to execute `tests/compare_python_rust.py`; no D2 matrix has been run. **Phase 7.10 in `docs/RUST_PORT_TODO.md` remains unchecked.**
- **Docker:** fresh image build and runtime are **unverified**. The existing `Dockerfile` references the GitHub ORT 1.24.2 archive URL that returned HTTP 404 in GitHub-hosted validation; this is a concrete **risk** requiring a separate Docker build and potential packaging change, not yet a measured Docker test failure.
- **systemd:** no suitable systemd-capable host/VM runtime test executed. Keep A3.4 open.
- **Pre-fix reproduction (C1.2–C1.3):** deterministic post-fix starvation tests pass; the pre-fix binary's failure and pre-change performance profile have not been measured.
- **Admission limits (C2.5):** measured low/moderate load does not establish whether additional bounded backpressure is required; do not silently introduce limits.
- **Final sign-off:** no claim of Python drop-in replacement readiness or production cutover. Cited green runs are real audio/backend successes, not Python parity results.

## 2026-11-18 — Docker packaging remediation and runtime smoke attempt

- Container source now uses `rust:1.100.0-bookworm` to align the builder with pinned CI and `python:3.13-slim-bookworm` as a temporary ONNX Runtime extraction stage. It downloads version-pinned `onnxruntime==1.24.2` from PyPI, copies only the native `libonnxruntime*.so*` libraries into the final Debian image, and does not ship Python in the runtime. This fixes the obsolete GitHub ONNX archive URL that returned HTTP 404.
- `.github/workflows/ci.yml` has an opt-in `run_docker` dispatch input and a `[run-docker]` master-commit marker (alongside required `[run-real-model]`). It builds with `docker buildx`, pins the backend checkout, mounts the verified model assets into a running container, checks loaded-model `/healthz`, and synthesizes actual WAV using two HTTP requests. Never infer Docker runtime readiness from the build alone.
- Systemd installation steps in `README.md` now extract the pinned ONNX 1.24.2 library from the PyPI wheel instead of the broken upstream release archive; this documentation change is not an observed systemd runtime test.
- `b10f0185db4ab63009bfd47af5082637408fe668`: first Docker opt-in [run #36](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44421144353) had not completed when reviewed, with the initial validation job held at runner dependency installation. **No Docker acceptance proof from that run.**
- `be6e8a400f733105769ce6768bffbc096f56deda`: normal push [run #37](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44422393483) **passed** the regular validation suite with the updated Dockerfile and README; no Docker job ran because the commit did not request it.
- Next action: trigger and inspect another explicitly opt-in Docker test on exact current `master`; only check A3.3 if that model-backed image test passes. Unverified outcomes stay open.


## 2026-11-19 — Ralph Bridge resumed; Docker acceptance verified

- Ralph Bridge reconnected and read `master` at `64b3cc73cace2e2a67848f228c92df4745658819`. No concurrent repository changes were observed at the start of this pass.
- [Run #36](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44421144353) (`b10f0185db4ab63009bfd47af5082637408fe668`) and [run #38](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44422566763) (`64b3cc73cace2e2a67848f228c92df4745658819`) both **completed successfully** after the earlier mid-run snapshots. Both had `validate`, `verify_release` and `real_model` jobs pass.
- Inspected run #38 `real_model` job and step status via Ralph Bridge. The `Build Docker image and test actual model-backed speech` step was **success**: image built, container started with real mounted model, loaded-model health checked, and **2/2 real synthesized WAV requests** verified (concurrency 1; wall 0.99 s, p50 0.4925 s, p95 0.6234 s). Docker acceptance **A3.3 is complete** for this tested image/host, not general production deployment.
- The canonical Python compatibility source is `ekkus93/KittenTTS_server` (from user project records), but **Ralph Bridge rejects direct get_repo_state/get_file reads with INVALID_ARGUMENT**; its exact SHA and executable reference remain unresolved. No D1.1/D2 comparison should be marked.
- Systemd remains unverified at this point. To avoid requiring manual deployment merely to test systemd's lifecycle, this commit adds a separate explicitly opt-in real-model systemd scenario to GitHub-hosted Ubuntu, requiring `systemd` as PID 1 and exercising the actual sample unit, installed service account, pinned ORT and model files, synthesized WAV, restart, health and stop. This is an **untested implementation pending CI**; A3.4 remains open until the test passes.


### CI #39 confirms real systemd service lifecycle

- [Run #39](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44494955365) on server SHA **`651761ff18fd359e73aac45bcfe62da7f29a0f3d`**: **PASS** `validate`, `verify_release`, and `real_model` jobs on the GitHub-hosted Ubuntu 26.04 VM. Actual sample `config/systemd/kittentts-server-rs.service` installed under `/etc/systemd/system`, with unprivileged `kittentts-server` account, copied verified model assets and ORT 1.24.2 native libraries, environment file and settings.json. Test explicitly required systemd as PID 1, started unit, validated loaded-model `/healthz`, produced **2/2 real WAV responses**, restarted the unit and rechecked health, stopped it and asserted inactive, and captured bounded `journalctl` diagnostics. All systemd-related test steps passed.
- The model-backed systemd test log reported 2/2 WAV calls in 0.68 s wall time; p50 0.3393 s and p95 0.3451 s on this CI runner. This is only a bounded smoke check, not a claim of production SLA or verification on a specific user's physical host.
- A3.4 **complete** for the documented installation and lifecycle on the test VM. Both A3.3 and A3.4 are now supported by actual CI runtime evidence.
- **Python follow-up:** canonical repository `ekkus93/KittenTTS_server` still cannot be read through Ralph Bridge's current repo scope; direct `get_repo_state` and `get_file` returned `INVALID_ARGUMENT`. A separate opt-in, read-only GitHub Actions reference probe was added to resolve the Python source's exact default branch, commit, dependency manifests, and entrypoint metadata if publicly accessible. The probe is not a Python parity test and must not close D1.1 or D2 unless actual comparison is subsequently executed.


## 2026-11-19 — Full Python-vs-Rust real-model API parity: default, auth and strict

### Exact inputs and execution
- Original Python reference: `ekkus93/KittenTTS_server` master, pinned **`0e49fd2ae1baa3f2c9b007c12a962b5d99bb55c7`**. Probe and checkout verified via GitHub Actions; Python 3.11, dependencies installed from its `requirements.txt` in an isolated virtual environment (FastAPI/Uvicorn/Pydantic/KittenTTS wheel). Backend: `ekkus93/kitten_tts_rs` **`df7718161bc68c8b13bea85e879308becf12e87b`**. Model: `KittenML/kitten-tts-nano-0.8-fp32` at **`e73cfb095bd949c786ccdcf3b8ab04eb36c47442`**. ORT: **1.24.2**.
- Rust comparison source commit: **`8c67a5b85cd092a6940693c5b60c00a4ef19c094`**, Python reference unchanged. GitHub Actions [run #53](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44634413093) **PASS**, including formatting, Clippy, feature compilation, `cargo test --all-features`, optimized binary build, live-model HTTP smoke, and three separate Python-vs-Rust sessions.
- Shared runtime settings: localhost, 24 kHz mono, default `jasper`, matching `Narrator` → `Jasper` voice alias, same model assets and API contract. Auth mode enabled with **test-only** key (never a production secret); strict mode enabled in an independent pair of processes. All sessions checked `/healthz` `model_loaded=true` on **both** real servers before comparing. Each case validates response status, media type, format headers, normalized error JSON, request-ID presence and WAV/PCM structural correctness. Audio sample counts, transfer framing and engine diagnostic fields are recorded as observations and are **not** required to be byte-for-byte identical.

### Dated per-case response matrix (run #53)
| Case | Default | Auth enabled | Strict mode |
| --- | --- | --- | --- |
| `healthz` | PASS | PASS | PASS |
| `voices` | PASS | PASS | PASS |
| `eleven_default` | PASS | PASS | PASS |
| `eleven_named` | PASS | PASS | PASS |
| `eleven_alias` | PASS | PASS | PASS |
| `eleven_unknown` | PASS | PASS | PASS |
| `stream_wav` | PASS | PASS | PASS |
| `stream_pcm` | PASS | PASS | PASS |
| `openai_wav` | PASS | PASS | PASS |
| `openai_pcm` | PASS | PASS | PASS |
| `empty_text` | PASS | PASS | PASS |
| `invalid_openai_model` | PASS | PASS | PASS |
| `invalid_format` | PASS | PASS | PASS |
| `long_text` | PASS | PASS | PASS |
| `speed` | PASS | PASS | PASS |
| `missing_auth` | — | PASS | — |
| `wrong_auth` | — | PASS | — |
| `conflicting_auth` | — | PASS | — |
| `strict_extra_fields` | — | — | PASS |

**Aggregates:** default **15 PASS / 0 FAIL / 0 BLOCKED**; authenticated **18 / 0 / 0**; strict **16 / 0 / 0**; total **49 PASS / 0 FAIL / 0 BLOCKED**. JSON case-by-case artifacts `parity-default.json`, `parity-authenticated.json`, `parity-strict.json` and bounded process logs are uploaded in [run #53](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44634413093). This completes D1.1, D2.1, D2.2, D2.3, D2.5, D2.8 for the **measured** configurations; it is not a claim of universal feature equivalence.

### Material mismatches identified and resolved (D3.1)
1. Original model schema mismatch: [run #43](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44498061254), default mode **14 PASS / 1 FAIL**, Python invalid OpenAI model → HTTP **422**, code `validation_error`, type `invalid_request_error`, generic `Request validation failed`; Rust returned 400 and exposed Serde's enum text. Changed **only request-schema** errors on Rust OpenAI route to match Python, preserving domain error HTTP 400. Verified in run #46 (15/15 default).
2. Strict unsupported format: [run #48](https://github.com/ekkus93/KittenTTS_rust_server/actions/runs/44632810849), default 15/15 and auth 18/18; strict **15 PASS / 1 FAIL**, Python `mp3` → `unsupported_output_format`, `details.output_format="mp3"`; Rust used `validation_error` without details. Added matching Rust error code/details for strict ElevenLabs and pseudo-stream routes, updated older Rust integration tests, and reran run #53: **all strict cases pass**.
3. Validation infrastructure issues during this iteration: Python venv interpreter symlink was incorrectly dereferenced in the multi-mode runner, bypassing its installed packages (run #47); fixed using an absolute non-resolved executable path. Test expectations for a dynamic request ID and Rust formatting were corrected. These are recorded as harness problems, **not** Python-versus-Rust compatibility failures.
4. No material differences remain among the **49 tested cases**. No waivers were requested or silently granted; untested variants remain explicit follow-up tasks. Historical ONNX download failures were fixed and revalidated in real model CI, supporting B2.7.

### Still unverified after this stage
- D2.4: invalid OpenAI **input** and additional schema errors have not been compared; do not equate the invalid-model case with the entire OpenAI request-validation matrix.
- D2.6: custom sample rate/channel and config JSON-vs-env precedence have not yet been compared. Strict extra-field behavior **was** compared.
- D2.7: the long-text and speed scenarios passed, but no explicit multi-chunk boundary/subjective audio-quality evaluation has been performed.
- D3.2–D3.4, G4–G5 and original Phase 7.10: **remain open** pending fuller compatibility and explicit sign-off. There is no claim of production-cutover readiness solely from run #53.
