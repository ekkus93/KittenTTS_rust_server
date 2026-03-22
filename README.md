# KittenTTS_rust_server

Compatibility-focused Rust port scaffold for `KittenTTS_server`.

## Phase 0 scaffold

This repository now contains the initial `kittentts-server-rs` crate scaffold with:

- a standalone Axum server package
- the locked `src/` module tree for routes, services, backend, middleware, models, config, and errors
- a working `/healthz` route
- default settings plus optional JSON/env config loading

## Run

```bash
cargo run --features real-backend
```

The server listens on `127.0.0.1:8008` by default.
With the real backend enabled, startup fails fast if the backend cannot initialize.
If `model_dir` is set, the server loads `config.json`, the ONNX model, and
`voices.npz` from that directory. Otherwise it follows the Python path and
downloads the default model repo from Hugging Face into a local cache.

On Linux, the backend now uses ONNX Runtime dynamic loading instead of `ort`'s
default static downloaded archive, because the prebuilt static bundle can fail
to link on older glibc hosts. Startup first respects `ORT_DYLIB_PATH`, then
looks for a local install under `~/.local/share/onnxruntime/<version>/libonnxruntime.so*`,
and finally falls back to the system library path if available. You can still
override the choice explicitly, for example:

```bash
ORT_DYLIB_PATH="$HOME/.local/share/onnxruntime/1.24.2/libonnxruntime.so.1.24.2" cargo run --features real-backend
```

## Config

- Example config: `config/settings.example.json`
- Environment variable prefix: `KITTENTTS_SERVER_`
- Optional backend asset directory override: `model_dir` or `KITTENTTS_SERVER_MODEL_DIR`
- Default backend repo when no local directory is configured: `KittenML/kitten-tts-nano-0.8`
- Cache path resolution: `HF_HOME`, then `XDG_CACHE_HOME`, then `HOME/.cache/huggingface/hub`
- ONNX Runtime shared library path override: `ORT_DYLIB_PATH`

## Validation

```bash
cargo fmt --check
cargo check --features real-backend
cargo test
```
