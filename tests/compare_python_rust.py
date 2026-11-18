#!/usr/bin/env python3
"""Compare live Python and Rust KittenTTS HTTP contracts without waveform hashes.

Start both services independently with equivalent models, settings and secrets.
Example:
  python3 tests/compare_python_rust.py \\
      --python-url http://127.0.0.1:8008 --rust-url http://127.0.0.1:8009 \\
      --python-ref EXACT_SHA --backend-ref EXACT_SHA --output parity.json

Results are evidence, not authorization to mark Phase 7.10 complete.
"""

import argparse
import json
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from live_backend_smoke import validate_pcm, validate_wav


def request(base, case, timeout):
    data = json.dumps(case["json"]).encode() if case.get("json") is not None else None
    headers = {"Content-Type": "application/json"} if data is not None else {}
    headers.update(case.get("headers", {}))
    req = Request(base.rstrip("/") + case["path"], data=data,
                  headers=headers, method=case.get("method", "GET"))
    start = time.monotonic()
    try:
        with urlopen(req, timeout=timeout) as response:
            status, raw_headers, body = response.status, response.headers, response.read(32 * 1024 * 1024 + 1)
    except HTTPError as error:
        status, raw_headers, body = error.code, error.headers, error.read(32 * 1024 * 1024 + 1)
    if len(body) > 32 * 1024 * 1024:
        raise RuntimeError("body exceeds 32 MiB safety limit")
    return status, raw_headers, body, round(time.monotonic() - start, 4)


def normalized_json(value):
    if isinstance(value, dict):
        return {key: normalized_json(inner) for key, inner in sorted(value.items())
                if key not in ("request_id", "timestamp")}
    if isinstance(value, list):
        return [normalized_json(item) for item in value]
    return value


def summarize(case, result, rate):
    status, headers, body, elapsed = result
    kind = headers.get_content_type()
    output = {
        "status": status,
        "content_type": kind,
        "x_output_format": headers.get("X-Output-Format"),
        "request_id_present": bool(headers.get("X-Request-Id")),
        "elapsed_s": elapsed,
        "content_length": headers.get("Content-Length"),
        "transfer_encoding": headers.get("Transfer-Encoding"),
    }
    if kind == "audio/wav" and status == 200:
        output["frames"] = validate_wav(body, channels=1, rate=rate)
        output["audio_kind"] = "wav"
    elif kind == "audio/pcm" and status == 200:
        output["frames"] = validate_pcm(body, channels=1, rate=rate)
        output["audio_kind"] = "pcm"
    else:
        try:
            doc = normalized_json(json.loads(body))
            if case["name"] == "healthz":
                # Known engine/version/ORT diagnostics differ by implementation.
                keep = ("status", "model_loaded", "default_voice_id",
                        "output_format", "sample_rate", "channel_layout")
                output["json"] = {key: doc.get(key) for key in keep if key in doc}
                output["engine_metadata"] = {
                    key: doc.get(key) for key in ("engine", "engine_version",
                                                "onnx_runtime_source", "onnx_runtime_path")
                    if key in doc
                }
            elif case["name"] == "voices" and isinstance(doc, dict):
                voices = doc.get("voices")
                if isinstance(voices, list):
                    doc["voices"] = sorted(voices, key=lambda v: json.dumps(v, sort_keys=True))
                output["json"] = doc
            else:
                output["json"] = doc
        except (ValueError, UnicodeDecodeError):
            output["body_preview"] = body[:100].hex()
    return output


def compare_signatures(case, py, rs):
    mismatches = []
    for key in ("status", "content_type", "x_output_format", "request_id_present", "audio_kind", "json"):
        if py.get(key) != rs.get(key):
            mismatches.append({"field": key, "python": py.get(key), "rust": rs.get(key)})
    # The Python shim and Rust pseudo-stream may differ in HTTP transfer framing.
    # Record but do not claim byte-identical audio or equivalent latency.
    observations = {}
    for key in ("frames", "content_length", "transfer_encoding", "engine_metadata"):
        if py.get(key) != rs.get(key):
            observations[key] = {"python": py.get(key), "rust": rs.get(key)}
    return ("FAIL" if mismatches else "PASS"), mismatches, observations


def corpus(auth_mode, strict):
    words = "This is a short speech parity test."
    eleven = {"text": words, "model_id": "kitten-local"}
    openai = {"model": "tts-1", "voice": "Jasper", "input": words}
    cases = [
        {"name": "healthz", "path": "/healthz"},
        {"name": "voices", "path": "/v1/voices"},
        {"name": "eleven_default", "method": "POST", "path": "/v1/text-to-speech", "json": eleven},
        {"name": "eleven_named", "method": "POST", "path": "/v1/text-to-speech/Jasper", "json": eleven},
        {"name": "eleven_unknown", "method": "POST", "path": "/v1/text-to-speech/unknown-voice", "json": eleven},
        {"name": "stream_wav", "method": "POST", "path": "/v1/text-to-speech/Jasper/stream",
         "json": {**eleven, "output_format": "wav_24000"}},
        {"name": "stream_pcm", "method": "POST", "path": "/v1/text-to-speech/Jasper/stream",
         "json": {**eleven, "output_format": "pcm_24000"}},
        {"name": "openai_wav", "method": "POST", "path": "/v1/audio/speech",
         "json": {**openai, "response_format": "wav"}},
        {"name": "openai_pcm", "method": "POST", "path": "/v1/audio/speech",
         "json": {**openai, "response_format": "pcm"}},
        {"name": "empty_text", "method": "POST", "path": "/v1/text-to-speech", "json": {"text": ""}},
        {"name": "invalid_openai_model", "method": "POST", "path": "/v1/audio/speech",
         "json": {**openai, "model": "not-a-model"}},
        {"name": "invalid_format", "method": "POST", "path": "/v1/text-to-speech",
         "json": {**eleven, "output_format": "mp3"}},
        {"name": "long_text", "method": "POST", "path": "/v1/text-to-speech",
         "json": {**eleven, "text": (words + " ") * 10}},
        {"name": "speed", "method": "POST", "path": "/v1/text-to-speech/Jasper",
         "json": {**eleven, "voice_settings": {"speed": 1.2}}},
    ]
    if strict:
        cases.append({"name": "strict_extra_fields", "method": "POST",
                      "path": "/v1/text-to-speech", "json": {**eleven, "extra_field": True}})
    if auth_mode == "enabled":
        cases += [
            {"name": "missing_auth", "path": "/v1/voices"},
            {"name": "wrong_auth", "path": "/v1/voices", "headers": {"xi-api-key": "not-a-key"}},
            {"name": "conflicting_auth", "path": "/v1/voices", "headers":
             {"xi-api-key": "one", "Authorization": "Bearer two"}},
        ]
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python-url", required=True)
    parser.add_argument("--rust-url", required=True)
    parser.add_argument("--python-ref", default="UNRECORDED")
    parser.add_argument("--rust-ref", default="UNRECORDED")
    parser.add_argument("--backend-ref", default="UNRECORDED")
    parser.add_argument("--model-ref", default="UNRECORDED")
    parser.add_argument("--auth-mode", choices=("enabled", "disabled"), default="disabled")
    parser.add_argument("--api-key", help="common API key, required when auth is enabled")
    parser.add_argument("--strict", action="store_true", help="both servers run strict mode")
    parser.add_argument("--sample-rate", type=int, default=24000)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--output", default="parity-report.json")
    args = parser.parse_args()
    if args.auth_mode == "enabled" and not args.api_key:
        parser.error("--api-key is mandatory with --auth-mode enabled")
    cases = corpus(args.auth_mode, args.strict)
    report = {
        "metadata": {"python_ref": args.python_ref, "rust_ref": args.rust_ref,
                     "backend_ref": args.backend_ref, "model_ref": args.model_ref,
                     "auth_mode": args.auth_mode, "strict": args.strict,
                     "sample_rate": args.sample_rate},
        "results": [],
    }
    for case in cases:
        c = dict(case)
        if args.auth_mode == "enabled" and case["name"] not in (
                "missing_auth", "wrong_auth", "conflicting_auth", "healthz"):
            c["headers"] = {**c.get("headers", {}), "xi-api-key": args.api_key}
        record = {"name": case["name"]}
        try:
            py = summarize(c, request(args.python_url, c, args.timeout), args.sample_rate)
            rs = summarize(c, request(args.rust_url, c, args.timeout), args.sample_rate)
            outcome, mismatches, observations = compare_signatures(c, py, rs)
            record.update({"status": outcome, "mismatches": mismatches,
                           "observations": observations,
                           "python": py, "rust": rs})
        except Exception as error:
            record.update({"status": "BLOCKED", "reason": type(error).__name__ + ": " + str(error)})
        report["results"].append(record)
        print(record["status"], record["name"])
    with open(args.output, "w", encoding="utf-8") as fp:
        json.dump(report, fp, indent=2, sort_keys=True)
        fp.write("\n")
    counts = {key: sum(r["status"] == key for r in report["results"])
              for key in ("PASS", "FAIL", "BLOCKED")}
    print("Summary:", counts, "Report:", args.output)
    if counts["FAIL"] or counts["BLOCKED"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
