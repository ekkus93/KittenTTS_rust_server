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

## Manual Compatibility Validation

On 2026-03-22, the Rust server and the Python compatibility target were run
side by side on localhost and exercised with the same request bodies for:

- `GET /healthz`
- `GET /v1/voices`
- `POST /v1/text-to-speech`
- `POST /v1/text-to-speech/jasper`
- `POST /v1/text-to-speech/not-a-real-voice`
- `POST /v1/text-to-speech/jasper/stream`
- `POST /v1/audio/speech` with both `wav` and `pcm`

Observed compatibility points:

- All compared ElevenLabs-style and OpenAI-style synthesis routes returned the
	same success status codes when given valid request bodies.
- `GET /v1/voices` returned the same JSON shape and the same voice inventory;
	the only observed difference was object key ordering inside nested `labels`
	maps.
- All successful WAV responses from both implementations were valid RIFF/WAVE
	PCM audio at 24 kHz mono.
- The OpenAI `pcm` route returned raw PCM payloads with matching route-level
	behavior and headers (`content-type: audio/pcm`, `X-Output-Format: pcm`).
- Default voice routing, explicit `jasper`, and unknown-voice fallback all
	returned `200` on both servers. Within each implementation, those three
	requests produced same-sized outputs, which is consistent with the expected
	fallback-to-default voice behavior.

Intentional or currently accepted differences observed during the manual run:

- `GET /healthz` is not byte-for-byte identical. The Rust server reports
	`engine: "kitten_tts_rs"`, currently exposes `engine_version: null`, and adds
	`onnx_runtime_source` plus `onnx_runtime_path`. The Python server reports
	`engine: "KittenTTS"` and `engine_version: "0.8.1"` without the ONNX Runtime
	fields.
- Audio bytes are not identical between the Rust and Python implementations for
	the same synthesis request, and the resulting payload sizes differ in this
	environment. The route behavior, media types, and container validity matched,
	but exact waveform parity was not observed.
- In this manual run, the Rust pseudo-stream route explicitly responded with
	chunked transfer encoding, while the Python route returned the same media type
	and successful body without advertising a matching `Content-Length` header.

The manual OpenAI comparison first surfaced a request-validation mismatch caused
by using `model: "kitten-local"` in the probe payload. Re-running the same
requests with the valid OpenAI-compatible model identifier `tts-1` produced
successful `wav` and `pcm` responses from both implementations.

## Local Run

Run the compatibility server directly from this repo:

```bash
cargo run --release --features real-backend
```

If you want to force a local model directory instead of the default Hugging Face
download/cache path, point `KITTENTTS_SERVER_MODEL_DIR` at a directory that
contains `config.json`, the ONNX model file referenced by that config, and
`voices.npz`:

```bash
KITTENTTS_SERVER_MODEL_DIR="$PWD/models/kitten-tts-nano" cargo run --release --features real-backend
```

You can also point the binary at a specific JSON config file with
`KITTENTTS_SERVER_CONFIG_FILE=/path/to/settings.json`.

## Docker

This repository now includes a sample [Dockerfile](Dockerfile) for Linux
container deployment and a matching [compose.yaml](compose.yaml).

Because the server intentionally keeps `kitten_tts_rs` as a sibling local path
dependency, the Docker build uses a named BuildKit context for that repo rather
than rewriting the dependency to a git or registry source.

Build the image from the Rust server repo root:

```bash
docker build --build-context kitten_tts_rs=../kitten_tts_rs -t kittentts-server-rs:latest .
```

The image uses a multi-stage build:

- the builder stage compiles `kittentts-server-rs` with `--features real-backend`
- the runtime stage installs `espeak-ng` and ships a pinned ONNX Runtime shared library at `~/.local/share/onnxruntime/1.24.2/`
- the final container runs as a non-root `elkitten` user and exposes a `/healthz` health check

`espeak-ng` is handled differently for the two supported deployment paths:

- Docker: the final runtime image installs `espeak-ng` directly, so the packaged container is self-contained for that dependency.
- systemd/local host deployment: `espeak-ng` remains a required host package and must be installed before the service is started.

Run the container with the default in-repo config directory:

```bash
docker run --rm \
	-p 8008:8008 \
	--cap-drop=ALL \
	--security-opt no-new-privileges \
	-v "$PWD/config:/app/config:ro" \
	-v kittentts-rs-hf-cache:/home/elkitten/.cache/huggingface \
	--name kittentts-server-rs \
	kittentts-server-rs:latest
```

The container forces `KITTENTTS_SERVER_HOST=0.0.0.0` so the published port is
reachable from the host. If `config/settings.json` is present in the mounted
`config/` directory, the server loads it automatically. Otherwise it uses the
same default settings and env overrides as the local binary.

If you want to mount your own model assets instead of letting the container use
the Hugging Face cache, add a bind mount plus `KITTENTTS_SERVER_MODEL_DIR`:

```bash
docker run --rm \
	-p 8008:8008 \
	--cap-drop=ALL \
	--security-opt no-new-privileges \
	-e KITTENTTS_SERVER_MODEL_DIR=/app/models/kitten-tts-nano \
	-v "$PWD/config:/app/config:ro" \
	-v "$PWD/models:/app/models:ro" \
	--name kittentts-server-rs \
	kittentts-server-rs:latest
```

If you want the container to restart automatically, add a Docker restart policy
such as `--restart unless-stopped`.

## Docker Compose

The compose file builds the same multi-stage image, binds the service to
`127.0.0.1:8008`, mounts the repo `config/` directory read-only, persists the
Hugging Face cache in a named volume, and applies the same basic hardening
flags.

Start the service:

```bash
docker compose up --build -d
```

Follow the logs:

```bash
docker compose logs -f
```

Stop and remove the container:

```bash
docker compose down
```

If you want to use host-managed model assets with compose, uncomment the model
volume line in [compose.yaml](compose.yaml) and set
`KITTENTTS_SERVER_MODEL_DIR=/app/models/...` in the service environment.

## systemd

For Linux hosts that should keep the Rust shim running in the background, a
sample unit file is provided at
[config/systemd/kittentts-server-rs.service](config/systemd/kittentts-server-rs.service).

1. Install the required host dependency.

```bash
sudo apt-get update
sudo apt-get install -y espeak-ng
```

This host-level install is the intended systemd packaging model for `espeak-ng`.

2. Build the release binary from this repo.

```bash
cargo build --release --features real-backend
```

3. Create the service account and deployment directories.

```bash
sudo useradd --system --home-dir /opt/kittentts-server-rs --shell /usr/sbin/nologin kittentts-server
sudo install -d -o kittentts-server -g kittentts-server \
	/opt/kittentts-server-rs/bin \
	/opt/kittentts-server-rs/config \
	/opt/kittentts-server-rs/.cache/huggingface
```

4. Install the binary and sample config.

```bash
sudo install -m 0755 target/release/kittentts-server-rs /opt/kittentts-server-rs/bin/kittentts-server-rs
sudo install -m 0644 config/settings.example.json /opt/kittentts-server-rs/config/settings.example.json
```

5. Optionally create `/opt/kittentts-server-rs/config/settings.json` from the
sample config. The unit points `KITTENTTS_SERVER_CONFIG_FILE` at that path.

6. If ONNX Runtime is not installed system-wide, install a compatible shared
library and point `ORT_DYLIB_PATH` at it through `/etc/default/kittentts-server-rs`.
For example:

```bash
sudo install -d /opt/kittentts-server-rs/lib/onnxruntime/1.24.2
curl -L --fail -o /tmp/onnxruntime-linux-x64-1.24.2.tgz \
	https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-linux-x64-1.24.2.tgz
sudo tar -xzf /tmp/onnxruntime-linux-x64-1.24.2.tgz --strip-components=2 \
	-C /opt/kittentts-server-rs/lib/onnxruntime/1.24.2 \
	onnxruntime-linux-x64-1.24.2/lib/libonnxruntime.so \
	onnxruntime-linux-x64-1.24.2/lib/libonnxruntime.so.1.24.2
printf '%s\n' 'ORT_DYLIB_PATH=/opt/kittentts-server-rs/lib/onnxruntime/1.24.2/libonnxruntime.so.1.24.2' \
	| sudo tee /etc/default/kittentts-server-rs >/dev/null
```

You can also add other runtime overrides such as `KITTENTTS_SERVER_LOG_LEVEL`
or `KITTENTTS_SERVER_MODEL_DIR` to the same environment file.

7. Copy the unit into systemd's unit directory.

```bash
sudo cp config/systemd/kittentts-server-rs.service /etc/systemd/system/kittentts-server-rs.service
```

8. Reload systemd and enable the service.

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now kittentts-server-rs
```

9. Check service status.

```bash
sudo systemctl status kittentts-server-rs
```

## Uninstall systemd Service

If you want to remove the systemd deployment later, stop and disable the unit
first:

```bash
sudo systemctl disable --now kittentts-server-rs
```

Then remove the installed unit and reload systemd:

```bash
sudo rm -f /etc/systemd/system/kittentts-server-rs.service
sudo systemctl daemon-reload
sudo systemctl reset-failed
```

If you also want to remove the deployed binary, config, cached model data, and
service account, remove the deployment directory and the user/group:

```bash
sudo rm -rf /opt/kittentts-server-rs
sudo userdel kittentts-server 2>/dev/null || true
sudo groupdel kittentts-server 2>/dev/null || true
sudo rm -f /etc/default/kittentts-server-rs
```

If you installed ONNX Runtime or model assets in a different location, remove
those paths separately.

The sample unit assumes:

- the deployment root is `/opt/kittentts-server-rs`
- the binary is `/opt/kittentts-server-rs/bin/kittentts-server-rs`
- the runtime config file is `/opt/kittentts-server-rs/config/settings.json`
- optional runtime overrides live in `/etc/default/kittentts-server-rs`
- the service runs as a dedicated `kittentts-server` user and group
