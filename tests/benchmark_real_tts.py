#!/usr/bin/env python3
"""Opt-in real-model HTTP concurrency measurement; expects a running Rust server.

Example:
  python3 tests/benchmark_real_tts.py --url http://127.0.0.1:8008 \\
      --requests 8 --concurrency 1 2 4 --pid 12345 --output /tmp/kittentts-bench.json

This script does not use a mock synthesizer. CPU/model/runner details must be
recorded alongside results before comparing runs. Times are descriptive.
"""

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import json
import math
import os
import platform
import statistics
import threading
import time
from urllib.request import Request, urlopen

from live_backend_smoke import validate_wav


def percentile(samples, fraction):
    ordered = sorted(samples)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def rss_kib(pid):
    if not pid:
        return None
    try:
        with open(f"/proc/{pid}/status", encoding="utf-8") as fp:
            for line in fp:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
    except (FileNotFoundError, PermissionError, ValueError, OSError):
        pass
    return None


def send_request(base, timeout, index, key):
    payload = json.dumps({
        "text": f"Short synthesis benchmark request number {index}.",
        "model_id": "kitten-local",
    }).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if key:
        headers["xi-api-key"] = key
    request = Request(base.rstrip("/") + "/v1/text-to-speech",
                      method="POST", data=payload, headers=headers)
    start = time.perf_counter()
    with urlopen(request, timeout=timeout) as response:
        if response.status != 200:
            raise RuntimeError(f"unexpected HTTP status {response.status}")
        audio = response.read(32 * 1024 * 1024 + 1)
        if len(audio) > 32 * 1024 * 1024:
            raise RuntimeError("response exceeded 32 MiB")
        frames = validate_wav(audio)
    return {"index": index, "latency_s": time.perf_counter() - start, "frames": frames}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True)
    parser.add_argument("--requests", type=int, default=8)
    parser.add_argument("--concurrency", type=int, nargs="+", default=[1, 2, 4])
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--pid", type=int, help="local server PID for Linux RSS observations")
    parser.add_argument("--model-ref", default="UNRECORDED")
    parser.add_argument("--server-ref", default="UNRECORDED")
    parser.add_argument("--backend-ref", default="UNRECORDED")
    parser.add_argument("--output", default="real-tts-benchmark.json")
    args = parser.parse_args()
    if not 2 <= args.requests <= 64 or any(not 1 <= x <= 8 for x in args.concurrency):
        parser.error("use 2-64 requests per level and concurrency 1-8")
    api_key = os.environ.get("KITTENTTS_API_KEY")
    report = {
        "date_utc": datetime.now(timezone.utc).isoformat(),
        "environment": {
            "system": platform.platform(),
            "machine": platform.machine(),
            "cpu_count": os.cpu_count(),
        },
        "server": args.url, "server_ref": args.server_ref,
        "backend_ref": args.backend_ref, "model_ref": args.model_ref,
        "requests_per_level": args.requests, "levels": [],
    }
    for workers in args.concurrency:
        finished = threading.Event()
        samples = []

        def observe_memory():
            while not finished.is_set():
                memory = rss_kib(args.pid)
                if memory is not None:
                    samples.append(memory)
                finished.wait(0.1)

        observer = threading.Thread(target=observe_memory, daemon=True)
        observer.start()
        failures = []
        durations = []
        start = time.perf_counter()
        with ThreadPoolExecutor(max_workers=workers) as executor:
            futures = {
                executor.submit(send_request, args.url, args.timeout, i, api_key): i
                for i in range(args.requests)
            }
            for future in as_completed(futures):
                try:
                    durations.append(future.result()["latency_s"])
                except Exception as error:
                    failures.append({"index": futures[future], "reason": str(error)})
        wall = time.perf_counter() - start
        finished.set()
        observer.join(timeout=1)
        result = {
            "concurrency": workers,
            "completed": len(durations),
            "failed": failures,
            "wall_time_s": round(wall, 4),
            "throughput_requests_s": round(len(durations) / wall, 4),
            "p50_latency_s": round(statistics.median(durations), 4) if durations else None,
            "p95_nearest_rank_latency_s": round(percentile(durations, 0.95), 4)
            if durations else None,
            "peak_observed_rss_kib": max(samples) if samples else None,
        }
        report["levels"].append(result)
        print(f"concurrency={workers}: {len(durations)}/{args.requests} passed; "
              f"wall={wall:.2f}s p50={result['p50_latency_s']} "
              f"p95={result['p95_nearest_rank_latency_s']}")
    with open(args.output, "w", encoding="utf-8") as fp:
        json.dump(report, fp, indent=2, sort_keys=True)
        fp.write("\n")
    if any(item["failed"] for item in report["levels"]):
        raise SystemExit(f"FAIL: see benchmark failures in {args.output}")
    print("PASS: real-model benchmark complete (do not infer universal throughput)")


if __name__ == "__main__":
    main()
