#!/usr/bin/env python3
"""Capture physical progress from the diagnostic H747 CM7/CM4 image pair.

The capture observes the legacy demonstration rings after a verified paired
flash. It proves that both cores crossed the current startup prerequisite and
made progress in both directions. It does not qualify MPY-08 transport
semantics, cache policy, reset recovery, signaling, or capacity.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import time
from typing import Any


MAILBOX_BASE = 0x3004_7000
CLOCK_READY_ADDRESS = MAILBOX_BASE + 0x1FC
CMD_CAPACITY = 16
EVT_CAPACITY = 8


class CaptureError(RuntimeError):
    """Raised when physical paired-core progress is not observed."""


def run(*args: str, cwd: pathlib.Path | None = None) -> str:
    """Run a command and return its stripped standard output."""

    completed = subprocess.run(
        args,
        cwd=cwd,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return completed.stdout.strip()


def sha256(path: pathlib.Path) -> str:
    """Return the SHA-256 digest of a file."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_words(
    *, chip: str, probe: str, speed: int, address: int, words: int
) -> list[int]:
    """Read 32-bit words from target RAM through probe-rs."""

    output = run(
        "probe-rs",
        "read",
        "--chip",
        chip,
        "--protocol",
        "swd",
        "--speed",
        str(speed),
        "--non-interactive",
        "--probe",
        probe,
        "--core",
        "0",
        "b32",
        hex(address),
        str(words),
    )
    values = re.findall(r"(?<![0-9a-fA-F])([0-9a-fA-F]{8})(?![0-9a-fA-F])", output)
    if len(values) < words:
        raise CaptureError(
            f"probe-rs returned {len(values)} words for {words}-word read at {address:#x}"
        )
    return [int(value, 16) for value in values[-words:]]


def ring_depth(head: int, tail: int) -> int:
    """Return a wrapping u32 producer-consumer distance."""

    return (head - tail) & 0xFFFF_FFFF


def sample(chip: str, probe: str, speed: int, elapsed_ms: int) -> dict[str, int]:
    """Capture the prerequisite flag and both ring index pairs."""

    cmd_head, cmd_tail = read_words(
        chip=chip,
        probe=probe,
        speed=speed,
        address=MAILBOX_BASE,
        words=2,
    )
    clock_ready, evt_head, evt_tail = read_words(
        chip=chip,
        probe=probe,
        speed=speed,
        address=CLOCK_READY_ADDRESS,
        words=3,
    )
    return {
        "elapsed_ms": elapsed_ms,
        "clock_ready": clock_ready,
        "cmd_head": cmd_head,
        "cmd_tail": cmd_tail,
        "cmd_depth": ring_depth(cmd_head, cmd_tail),
        "evt_head": evt_head,
        "evt_tail": evt_tail,
        "evt_depth": ring_depth(evt_head, evt_tail),
    }


def progressed(observation: dict[str, int]) -> bool:
    """Return whether the sample proves both producers and consumers ran."""

    return (
        observation["clock_ready"] == 1
        and observation["cmd_head"] > 0
        and observation["cmd_tail"] > 0
        and observation["cmd_depth"] <= CMD_CAPACITY
        and observation["evt_head"] > 0
        and observation["evt_tail"] > 0
        and observation["evt_depth"] <= EVT_CAPACITY
    )


def relative(path: pathlib.Path, root: pathlib.Path) -> str:
    """Return a repository-relative path where possible."""

    try:
        return str(path.resolve().relative_to(root.resolve()))
    except ValueError:
        return str(path.resolve())


def main() -> int:
    """Capture the physical diagnostic and write its evidence envelope."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chip", required=True)
    parser.add_argument("--probe", required=True)
    parser.add_argument("--speed", type=int, default=1000)
    parser.add_argument("--settle-seconds", type=float, default=5.0)
    parser.add_argument("--timeout-seconds", type=float, default=10.0)
    parser.add_argument("--sample-period", type=float, default=0.5)
    parser.add_argument("--static-evidence", required=True, type=pathlib.Path)
    parser.add_argument("--json-out", required=True, type=pathlib.Path)
    args = parser.parse_args()

    if args.settle_seconds < 0 or args.timeout_seconds <= 0 or args.sample_period <= 0:
        raise CaptureError("settle, timeout, and sample-period values must be positive")

    root = pathlib.Path(run("git", "rev-parse", "--show-toplevel"))
    static_evidence = args.static_evidence.resolve()
    if not static_evidence.is_file():
        raise CaptureError(f"static paired-image evidence is missing: {static_evidence}")

    time.sleep(args.settle_seconds)
    started = time.monotonic()
    samples: list[dict[str, int]] = []
    final: dict[str, int] | None = None
    while time.monotonic() - started <= args.timeout_seconds:
        elapsed_ms = round((time.monotonic() - started) * 1000)
        observation = sample(args.chip, args.probe, args.speed, elapsed_ms)
        samples.append(observation)
        if progressed(observation):
            final = observation
            break
        time.sleep(args.sample_period)

    if final is None:
        last = samples[-1] if samples else None
        raise CaptureError(f"paired-core ring progress was not observed; last={last}")

    commit = run("git", "rev-parse", "HEAD", cwd=root)
    tree_clean = not run(
        "git", "status", "--porcelain", "--untracked-files=all", cwd=root
    )
    source_paths = [
        root / "Makefile",
        root / "examples/stm32h747i-disco/memory.x",
        root / "examples/stm32h747i-disco/memory_cm4.x",
        root / "examples/stm32h747i-disco/src/ipc.rs",
        root / "examples/stm32h747i-disco/src/main.rs",
        root / "examples/stm32h747i-disco/src/cm4_main.rs",
        root / "scripts/capture_stm32h747i_disco_pair.py",
    ]
    now = dt.datetime.now(dt.timezone.utc)
    evidence: dict[str, Any] = {
        "schema_version": "mpy08-disco-physical-diagnostic-v1",
        "evidence_id": f"MPY08-DISCO-PHYSICAL-{commit[:12]}-{now.date().isoformat()}",
        "created_at": now.isoformat().replace("+00:00", "Z"),
        "qualification": "diagnostic_board_prerequisite",
        "profile": "stm32h747i-disco-legacy-mailbox-candidate",
        "normative_decision": False,
        "source": {
            "commit": commit,
            "tree_clean": tree_clean,
            "probe_sources": [
                {"path": relative(path, root), "sha256": sha256(path)}
                for path in source_paths
            ],
            "static_evidence": {
                "path": relative(static_evidence, root),
                "sha256": sha256(static_evidence),
            },
        },
        "toolchain": {
            "probe_rs_version": run("probe-rs", "--version").splitlines()[0],
            "probe_selector": args.probe,
            "swd_speed_khz": args.speed,
        },
        "environment": {
            "hardware_label": "STM32H747I-DISCO",
            "chip": args.chip,
            "physical_board": True,
        },
        "method": {
            "kind": "post_flash_shared_ring_observation",
            "settle_seconds": args.settle_seconds,
            "timeout_seconds": args.timeout_seconds,
            "sample_period_seconds": args.sample_period,
            "limitations": [
                "The observed rings use the legacy native-layout demonstration protocol.",
                "Counter progress does not qualify MPY canonical framing or operation semantics.",
                "A retained value cannot substitute for the still-open Boot Epoch/reset proof.",
                "Cache stress, stalls, wraparound, signaling loss, capacity, and physical input remain open.",
            ],
        },
        "samples": samples,
        "final": final,
        "checks": [
            {"name": "CM7 published clock-ready prerequisite", "passed": True},
            {"name": "CM4 produced and CM7 consumed a command", "passed": True},
            {"name": "CM7 produced and CM4 consumed a frame event", "passed": True},
            {"name": "observed command depth stayed within 16 slots", "passed": True},
            {"name": "observed event depth stayed within 8 slots", "passed": True},
        ],
    }

    destination = args.json_out.resolve()
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print(
        "PASS: STM32H747I-DISCO physical paired-core diagnostic "
        f"(cmd={final['cmd_head']}/{final['cmd_tail']}, "
        f"evt={final['evt_head']}/{final['evt_tail']})"
    )
    print(f"evidence={args.json_out}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError, CaptureError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        raise SystemExit(1) from error
