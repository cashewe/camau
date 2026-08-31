from __future__ import annotations

import argparse
import asyncio
import gc
import json
import os
import platform
import statistics
import sys
import time
from pathlib import Path
from typing import Any

import psutil

from camau import Router

OBJECT_BYTES = 10 * 1024


async def noop(payload: dict[str, Any]) -> dict[str, Any]:
    return payload


def linear_router() -> Router:
    nodes = []
    for index in range(10):
        node = {"id": f"node-{index}", "type": "task", "task": "noop"}
        if index < 9:
            node["next"] = f"node-{index + 1}"
        nodes.append(node)
    return Router({"entry": "node-0", "output": "node-9", "nodes": nodes}, {"noop": noop})


def fanout_router() -> Router:
    branches = [{"id": f"branch-{index}", "target": f"task-{index}"} for index in range(10)]
    tasks = [
        {"id": f"task-{index}", "type": "task", "task": "noop", "next": "join"}
        for index in range(10)
    ]
    inputs = {f"branch-{index}": f"task-{index}" for index in range(10)}
    specification = {
        "entry": "fan",
        "output": "join",
        "nodes": [
            {"id": "fan", "type": "fan-out", "branches": branches},
            *tasks,
            {"id": "join", "type": "converge", "inputs": inputs},
        ],
    }
    return Router(specification, {"noop": noop})


def percentile(samples: list[float], fraction: float) -> float:
    ordered = sorted(samples)
    return ordered[min(len(ordered) - 1, round((len(ordered) - 1) * fraction))]


async def measure(
    router: Router, payload: dict[str, Any], warmup: int, samples: int
) -> dict[str, float | int]:
    for _ in range(warmup):
        await router.run(payload)
    timings = []
    for _ in range(samples):
        started = time.perf_counter_ns()
        await router.run(payload)
        timings.append((time.perf_counter_ns() - started) / 1_000_000)
    return {
        "samples": samples,
        "warmup": warmup,
        "p50_ms": round(statistics.median(timings), 6),
        "p95_ms": round(percentile(timings, 0.95), 6),
    }


async def soak(router: Router, payload: dict[str, Any], runs: int) -> dict[str, int]:
    process = psutil.Process()
    gc.collect()
    before = process.memory_info().rss
    for _ in range(runs):
        await router.run(payload)
    gc.collect()
    after = process.memory_info().rss
    return {
        "runs": runs,
        "rss_before_bytes": before,
        "rss_after_bytes": after,
        "rss_delta_bytes": after - before,
    }


async def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--samples", type=int, default=2_000)
    parser.add_argument("--warmup", type=int, default=200)
    parser.add_argument("--soak-runs", type=int, default=100_000)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    payload = {"blob": "x" * OBJECT_BYTES}
    linear = linear_router()
    fanout = fanout_router()
    report = {
        "environment": {
            "platform": platform.platform(),
            "processor": platform.processor(),
            "python": sys.version.split()[0],
            "build_profile": "release"
            if not __debug__
            else os.environ.get("CAMAU_BUILD_PROFILE", "unspecified"),
        },
        "object_size_bytes": len(json.dumps(payload, separators=(",", ":")).encode()),
        "linear_10_node": await measure(linear, payload, args.warmup, args.samples),
        "fanout_10_branch": await measure(fanout, payload, args.warmup, args.samples),
        "soak_linear": await soak(linear, payload, args.soak_runs),
    }
    rendered = json.dumps(report, indent=2)
    print(rendered)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered + "\n", encoding="utf-8")


if __name__ == "__main__":
    asyncio.run(main())
