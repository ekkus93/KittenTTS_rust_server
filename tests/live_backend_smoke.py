#!/usr/bin/env python3
"""Opt-in, real-ONNX KittenTTS HTTP smoke test. No third-party Python dependencies.

This test starts the release server itself; fake synthesizers cannot satisfy it.
Build first: cargo build --release --features real-backend
Then: python3 tests/live_backend_smoke.py
Set ORT_DYLIB_PATH or install ONNX Runtime; provide model assets using
KITTENTTS_SERVER_MODEL_DIR or the documented Hugging Face cache/default.
"""

import argparse
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def exchange(url, method="GET", payload=None, timeout=120):
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    headers = {"Content-Type": "application/json"} if data is not None else {}
    request = Request(url, data=data, headers=headers, method=method)
    try:
        with urlopen(request, timeout=timeout) as response:
            return response.status, response.headers, response.read(32 * 1024 * 1024 + 1)
    except HTTPError as error:
        return error.code, error.headers, error.read(32 * 1024 * 1024 + 1)


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def validate_pcm(blob, channels=1, rate=24000):
    require(len(blob) >= channels * 2 * max(1, rate // 20), "audio shorter than 50ms")
    require(len(blob) % (2 * channels) == 0, "unaligned 16-bit PCM frames")
    peak = max(abs(sample[0]) for sample in struct.iter_unpack("<h", blob))
    require(peak >= 10, "audio is silent or nearly silent")
    return len(blob) // (2 * channels)


def validate_wav(blob, channels=1, rate=24000):
    require(len(blob) >= 44, "WAV too short")
    require(blob[:4] == b"RIFF" and blob[8:12] == b"WAVE", "invalid RIFF/WAVE")
    require(blob[12:16] == b"fmt " and blob[36:40] == b"data", "unsupported WAV layout")
    require(struct.unpack_from("<I", blob, 16)[0] == 16, "expected PCM fmt chunk")
    fmt, actual_channels, actual_rate = struct.unpack_from("<HHI", blob, 20)
    require(fmt == 1 and actual_channels == channels and actual_rate == rate, "unexpected WAV format")
    require(struct.unpack_from("<H", blob, 34)[0] == 16, "not 16-bit audio")
    data_len = struct.unpack_from("<I", blob, 40)[0]
    require(data_len == len(blob) - 44, "WAV data chunk length mismatch")
    return validate_pcm(blob[44:], channels=channels, rate=rate)


def ready(base, process, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"server exited during startup (code {process.returncode})")
        try:
            status, _headers, body = exchange(base + "/healthz", timeout=2)
            if status == 200:
                health = json.loads(body)
                require(health.get("model_loaded") is True, "backend not loaded")
                return
        except (OSError, URLError, ValueError):
            pass
        time.sleep(0.5)
    raise TimeoutError(f"real-backend startup exceeded {timeout}s")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/release/kittentts-server-rs")
    parser.add_argument("--startup-timeout", type=int, default=240)
    parser.add_argument("--request-timeout", type=int, default=120)
    args = parser.parse_args()
    binary = Path(args.binary).resolve()
    require(binary.is_file(), f"missing release binary: {binary}")
    port = free_port()
    base = f"http://127.0.0.1:{port}"
    env = os.environ.copy()
    # Isolate default config lookup so settings.json cannot silently win over
    # the test's localhost/port/auth configuration (JSON has final precedence).
    env.pop("KITTENTTS_SERVER_CONFIG_FILE", None)
    env.update({
        "KITTENTTS_SERVER_HOST": "127.0.0.1",
        "KITTENTTS_SERVER_PORT": str(port),
        "KITTENTTS_SERVER_AUTH_ENABLED": "false",
        "KITTENTTS_SERVER_SAMPLE_RATE": "24000",
        "KITTENTTS_SERVER_CHANNEL_LAYOUT": "mono",
    })
    with tempfile.TemporaryDirectory(prefix="kittentts-smoke-") as workdir:
        log_path = Path(workdir) / "server.log"
        with log_path.open("w+") as log:
            process = subprocess.Popen(
                [str(binary)], env=env, cwd=workdir, stdout=log, stderr=subprocess.STDOUT
            )
            try:
                ready(base, process, args.startup_timeout)
                status, _headers, body = exchange(base + "/v1/voices")
                require(status == 200 and len(json.loads(body).get("voices", [])) > 0,
                        "voices route failed or empty")
                print("PASS /healthz and /v1/voices (real model loaded)")
                payload = {"text": "Hello from the Rust speech server.", "model_id": "kitten-local"}
                cases = [
                    ("ElevenLabs default WAV", "/v1/text-to-speech", payload, "wav"),
                    ("ElevenLabs Jasper WAV", "/v1/text-to-speech/Jasper", payload, "wav"),
                    ("ElevenLabs unknown fallback WAV", "/v1/text-to-speech/not-a-real-voice", payload, "wav"),
                    ("ElevenLabs streamed WAV", "/v1/text-to-speech/Jasper/stream",
                     {**payload, "output_format": "wav_24000"}, "wav"),
                    ("ElevenLabs streamed PCM", "/v1/text-to-speech/Jasper/stream",
                     {**payload, "output_format": "pcm_24000"}, "pcm"),
                    ("OpenAI WAV", "/v1/audio/speech",
                     {"model": "tts-1", "voice": "Jasper", "input": payload["text"],
                      "response_format": "wav"}, "wav"),
                    ("OpenAI PCM", "/v1/audio/speech",
                     {"model": "tts-1", "voice": "Jasper", "input": payload["text"],
                      "response_format": "pcm"}, "pcm"),
                ]
                for title, path, body_json, kind in cases:
                    status, headers, audio = exchange(
                        base + path, "POST", body_json, args.request_timeout
                    )
                    require(status == 200, f"{title}: expected 200, got {status}: {audio[:200]!r}")
                    require(headers.get_content_type() == f"audio/{kind}",
                            f"{title}: wrong content type")
                    frames = validate_wav(audio) if kind == "wav" else validate_pcm(audio)
                    require(headers.get("X-Output-Format", "").startswith(kind),
                            f"{title}: missing X-Output-Format")
                    require(headers.get("X-Request-Id"), f"{title}: missing request ID")
                    print(f"PASS {title}: {frames} samples")
                for title, path, bad in [
                    ("empty ElevenLabs text", "/v1/text-to-speech", {"text": ""}),
                    ("invalid OpenAI model", "/v1/audio/speech",
                     {"model": "invalid", "voice": "Jasper", "input": "hello"}),
                ]:
                    status, _headers, body = exchange(
                        base + path, "POST", bad, args.request_timeout
                    )
                    require(status == 400 and b"error" in body,
                            f"{title}: expected 400 error, got {status}")
                    print(f"PASS {title} rejection")
            except BaseException:
                log.flush()
                log.seek(0)
                sys.stderr.write("Server diagnostic log (final 4000 characters):\n")
                sys.stderr.write(log.read()[-4000:] + "\n")
                raise
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
    print("PASS live model-backed HTTP smoke suite")


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, RuntimeError, TimeoutError, OSError) as error:
        sys.exit(f"FAIL: {error}")
