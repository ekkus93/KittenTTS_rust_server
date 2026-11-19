#!/usr/bin/env python3
"""Run separate pinned Python/Rust model-backed parity scenarios in CI.

Each mode launches fresh Python and Rust servers with identical non-secret
environment settings, validates loaded-model readiness, calls the existing
comparator, writes per-mode JSON evidence, and tears down both process groups.
Test output never includes raw audio or model weights.
"""
import argparse
import json
import os
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path
from urllib.error import URLError
from urllib.request import urlopen

AUTH_TEST_KEY = "ci-parity-test-only-key"
MODES = (
    # name, auth, strict, sample_rate, output_channels, JSON-over-env override
    ("default", False, False, 24000, 1, False),
    ("authenticated", True, False, 24000, 1, False),
    ("strict", False, True, 24000, 1, False),
    ("stereo_22050", False, False, 22050, 2, False),
    ("json_precedence", False, False, 22050, 2, True),
)


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def ready(port, proc, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"server exited with code {proc.returncode}")
        try:
            with urlopen(f"http://127.0.0.1:{port}/healthz", timeout=2) as response:
                health = json.load(response)
            if health.get("model_loaded") is True:
                return
        except (URLError, TimeoutError, ValueError, OSError):
            pass
        time.sleep(0.5)
    raise TimeoutError(f"model-backed readiness timed out on port {port}")


def stop(proc):
    if proc.poll() is None:
        try:
            os.killpg(proc.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait(timeout=5)
    else:
        proc.wait(timeout=5)


def run_mode(args, mode, auth, strict, rate, channels, json_precedence):
    a, b = free_port(), free_port()
    if a == b:
        b = free_port()
    source = Path(args.python_dir).resolve()
    binary = Path(args.rust_binary).resolve()
    # Do not dereference a venv's bin/python symlink: resolving it to the
    # system interpreter bypasses the venv's installed FastAPI/KittenTTS.
    python = Path(args.python_executable).absolute()
    resultdir = Path(args.output_dir).resolve()
    resultdir.mkdir(parents=True, exist_ok=True)
    base = dict(os.environ)
    base.pop("KITTENTTS_SERVER_CONFIG_FILE", None)
    base.update({
        "KITTENTTS_SERVER_HOST": "127.0.0.1",
        "KITTENTTS_SERVER_AUTH_ENABLED": "true" if auth else "false",
        "KITTENTTS_SERVER_LOCAL_API_KEY": AUTH_TEST_KEY,
        "KITTENTTS_SERVER_STRICT_MODE": "true" if strict else "false",
        "KITTENTTS_SERVER_SAMPLE_RATE": "24000" if json_precedence else str(rate),
        "KITTENTTS_SERVER_CHANNEL_LAYOUT": ("mono" if json_precedence
                                            else "stereo" if channels == 2 else "mono"),
        "KITTENTTS_SERVER_VOICE_MAP": '{"Narrator":"Jasper"}',
    })
    launched = []
    try:
        commands = (
            ("python", [str(python), "-m", "uvicorn", "app.main:app",
                        "--host", "127.0.0.1", "--port", str(a), "--no-server-header"],
             source, a),
            ("rust", [str(binary)], resultdir, b),
        )
        for label, command, cwd, port in commands:
            env = dict(base, KITTENTTS_SERVER_PORT=str(port))
            if json_precedence:
                # Deliberately conflict with environment values; the JSON file
                # must win (22050 Hz stereo over env 24000 Hz mono).
                configfile = resultdir / f"parity-{mode}-{label}.settings.json"
                configfile.write_text(
                    json.dumps({"sample_rate": rate,
                                "channel_layout": "stereo" if channels == 2 else "mono"}) + "\n",
                    encoding="utf-8",
                )
                env["KITTENTTS_SERVER_CONFIG_FILE"] = str(configfile)
            logfile = resultdir / f"parity-{mode}-{label}.log"
            handle = logfile.open("w", encoding="utf-8")
            try:
                proc = subprocess.Popen(
                    command, cwd=cwd, env=env, stdout=handle,
                    stderr=subprocess.STDOUT, start_new_session=True,
                )
            finally:
                handle.close()
            launched.append((label, proc, logfile, port))
        for label, proc, log_path, port in launched:
            try:
                ready(port, proc, args.startup_timeout)
            except (OSError, RuntimeError, TimeoutError) as error:
                # Bound diagnostics to prevent log leakage on CI failure.
                try:
                    diagnostics = log_path.read_text(encoding="utf-8")[-3000:]
                except OSError:
                    diagnostics = "<log unavailable>"
                print(f"{mode}: {label} startup failed: {error}", flush=True)
                print(f"{mode}: {label} diagnostics: {diagnostics}", flush=True)
                raise
            print(f"{mode}: {label} real model loaded", flush=True)
        cmd = [
            sys.executable, str(Path(__file__).with_name("compare_python_rust.py")),
            "--python-url", f"http://127.0.0.1:{a}",
            "--rust-url", f"http://127.0.0.1:{b}",
            "--python-ref", args.python_ref,
            "--rust-ref", args.rust_ref,
            "--backend-ref", args.backend_ref,
            "--model-ref", args.model_ref,
            "--timeout", str(args.request_timeout),
            "--sample-rate", str(rate), "--channels", str(channels),
            "--output", str(resultdir / f"parity-{mode}.json"),
        ]
        if auth:
            cmd += ["--auth-mode", "enabled", "--api-key", AUTH_TEST_KEY]
        if strict:
            cmd += ["--strict"]
        print(f"Running {mode} parity matrix", flush=True)
        result = subprocess.run(cmd, check=False, timeout=args.suite_timeout)
        return result.returncode == 0
    except (OSError, RuntimeError, TimeoutError, subprocess.TimeoutExpired) as exc:
        print(f"BLOCKED {mode}: {type(exc).__name__}: {exc}", flush=True)
        return False
    finally:
        for _label, proc, _log, _port in reversed(launched):
            stop(proc)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python-dir", required=True)
    parser.add_argument("--python-executable", required=True)
    parser.add_argument("--rust-binary", required=True)
    parser.add_argument("--python-ref", required=True)
    parser.add_argument("--rust-ref", required=True)
    parser.add_argument("--backend-ref", required=True)
    parser.add_argument("--model-ref", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--startup-timeout", type=int, default=240)
    parser.add_argument("--request-timeout", type=int, default=180)
    parser.add_argument("--suite-timeout", type=int, default=900)
    args = parser.parse_args()
    results = {}
    for mode, auth, strict, rate, channels, json_precedence in MODES:
        results[mode] = run_mode(args, mode, auth, strict, rate, channels, json_precedence)
    print("Parity modes: " + json.dumps(results, sort_keys=True), flush=True)
    if not all(results.values()):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
