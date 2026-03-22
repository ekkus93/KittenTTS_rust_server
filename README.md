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
cargo run
```

The server listens on `127.0.0.1:8008` by default.

## Config

- Example config: `config/settings.example.json`
- Environment variable prefix: `KITTENTTS_SERVER_`

## Validation

```bash
cargo fmt --check
cargo check
cargo test
```
