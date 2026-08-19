#!/usr/bin/env python3
"""Capture and validate CPY-03 bounded-channel capacity evidence."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import time
import tomllib
from pathlib import Path
from typing import Any, Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = REPO_ROOT / "docs/cpython/CPY-CAPACITY-EVIDENCE.schema.json"
DEFAULT_OUTPUT = REPO_ROOT / "docs/cpython/evidence/CPY-CAPACITY-HOST-2026-08-18.json"
GENERATED_DIR = REPO_ROOT / "docs/cpython/evidence/_generated"
PROBE_SOURCE = "runtime-std/examples/cpy_capacity_probe.rs"
PROBE_MANIFEST = "runtime-std/Cargo.toml"
ORCHESTRATOR_SOURCE = "scripts/cpy_capacity_probe.py"
SCENARIOS = ("cold-burst", "sustained", "observer-stall")
DEFAULT_CANDIDATES = ((8, 16, 4), (16, 32, 8), (32, 64, 16), (64, 128, 32))
LEGACY_SOURCE_PATHS = (
    PROBE_SOURCE,
    "runtime-std/src/lib.rs",
    PROBE_MANIFEST,
    ORCHESTRATOR_SOURCE,
    "docs/cpython/CPY-CAPACITY-EVIDENCE.schema.json",
)
SERVICE_SOURCE_PATHS = (
    PROBE_SOURCE,
    "runtime-std/src/lib.rs",
    "runtime-std/src/readiness.rs",
    "runtime-std/src/service.rs",
    PROBE_MANIFEST,
    ORCHESTRATOR_SOURCE,
    "docs/cpython/CPY-CAPACITY-EVIDENCE.schema.json",
)


class EvidenceError(RuntimeError):
    """Raised when capacity evidence is incomplete or inconsistent."""


def _run(arguments: list[str], *, text: bool = True) -> str | bytes:
    """Run one checked command in the rlvgl repository."""

    return subprocess.check_output(arguments, cwd=REPO_ROOT, text=text)


def _git(*arguments: str, text: bool = True) -> str | bytes:
    """Run one checked Git query in the rlvgl repository."""

    return _run(["git", *arguments], text=text)


def _sha256(data: bytes) -> str:
    """Return a manifest-formatted SHA-256 digest."""

    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def _load_json(path: Path) -> dict[str, Any]:
    """Load one UTF-8 JSON object."""

    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise EvidenceError(f"expected JSON object: {path}")
    return value


def _parse_candidate(value: str) -> tuple[int, int, int]:
    """Parse one ingress:egress:turn candidate tuple."""

    try:
        parts = tuple(int(part) for part in value.split(":"))
    except ValueError as error:
        raise argparse.ArgumentTypeError(f"invalid candidate {value!r}: {error}") from error
    if len(parts) != 3 or any(part <= 0 for part in parts):
        raise argparse.ArgumentTypeError(
            f"candidate {value!r} must be three positive integers: ingress:egress:turn"
        )
    return parts


def _candidate_id(candidate: tuple[int, int, int]) -> str:
    """Return the stable identifier for one candidate tuple."""

    ingress, egress, turn = candidate
    return f"i{ingress}-e{egress}-t{turn}"


def _require_clean_source() -> str:
    """Return HEAD only when every authored and untracked path is clean."""

    status = _git("status", "--porcelain", "--untracked-files=all")
    assert isinstance(status, str)
    if status:
        raise EvidenceError("capacity capture requires a clean rlvgl worktree")
    commit = _git("rev-parse", "HEAD")
    assert isinstance(commit, str)
    return commit.strip()


def _source_records(commit: str) -> list[dict[str, str]]:
    """Hash the probe's authored sources at one immutable commit."""

    records = []
    service_source = subprocess.run(
        ["git", "cat-file", "-e", f"{commit}:runtime-std/src/service.rs"],
        cwd=REPO_ROOT,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode == 0
    paths = SERVICE_SOURCE_PATHS if service_source else LEGACY_SOURCE_PATHS
    for path in paths:
        data = _git("show", f"{commit}:{path}", text=False)
        assert isinstance(data, bytes)
        records.append({"path": path, "sha256": _sha256(data)})
    return records


def _cargo_metadata() -> dict[str, Any]:
    """Load locked Cargo metadata for the committed probe graph."""

    raw = _run(["cargo", "metadata", "--locked", "--format-version", "1"])
    assert isinstance(raw, str)
    return json.loads(raw)


def _crossbeam_pin(lock_bytes: bytes) -> dict[str, str]:
    """Return the exact Crossbeam Channel version and registry checksum."""

    lock = tomllib.loads(lock_bytes.decode("utf-8"))
    packages = [
        package
        for package in lock.get("package", [])
        if package.get("name") == "crossbeam-channel"
    ]
    if len(packages) != 1:
        raise EvidenceError("expected exactly one crossbeam-channel package in Cargo.lock")
    package = packages[0]
    checksum = package.get("checksum")
    if not isinstance(checksum, str) or not re.fullmatch(r"[0-9a-f]{64}", checksum):
        raise EvidenceError("crossbeam-channel registry checksum is missing")
    return {
        "version": package["version"],
        "checksum": f"sha256:{checksum}",
    }


def _build_probe(metadata: dict[str, Any]) -> Path:
    """Build and return the native release probe executable."""

    subprocess.check_call(
        [
            "cargo",
            "build",
            "--locked",
            "--release",
            "-p",
            "rlvgl-runtime-std",
            "--example",
            "cpy_capacity_probe",
        ],
        cwd=REPO_ROOT,
    )
    suffix = ".exe" if os.name == "nt" else ""
    binary = Path(metadata["target_directory"]) / "release/examples" / f"cpy_capacity_probe{suffix}"
    if not binary.is_file():
        raise EvidenceError(f"probe executable missing after build: {binary}")
    return binary


def _read_rss_bytes(process_id: int) -> tuple[int | None, str]:
    """Sample resident bytes for one child without entering its address space."""

    if sys.platform.startswith("linux"):
        status = Path(f"/proc/{process_id}/status")
        try:
            text = status.read_text(encoding="utf-8")
        except (FileNotFoundError, ProcessLookupError):
            return None, "linux-proc-status-vmrss"
        match = re.search(r"^VmRSS:\s+(\d+)\s+kB$", text, re.MULTILINE)
        return (
            int(match.group(1)) * 1024 if match else None,
            "linux-proc-status-vmrss",
        )
    if sys.platform == "darwin":
        completed = subprocess.run(
            ["ps", "-o", "rss=", "-p", str(process_id)],
            check=False,
            capture_output=True,
            text=True,
        )
        value = completed.stdout.strip()
        return (int(value) * 1024 if value.isdigit() else None, "darwin-ps-rss")
    raise EvidenceError(f"no peak-RSS sampler for {sys.platform}")


def _run_probe(binary: Path, arguments: list[str]) -> tuple[dict[str, Any], int, str]:
    """Run one probe case while sampling whole-process peak RSS."""

    process = subprocess.Popen(
        [str(binary), *arguments],
        cwd=REPO_ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    peak_rss_bytes = 0
    rss_source = ""
    while process.poll() is None:
        sample, rss_source = _read_rss_bytes(process.pid)
        if sample is not None:
            peak_rss_bytes = max(peak_rss_bytes, sample)
        time.sleep(0.002)
    stdout, stderr = process.communicate()
    if process.returncode != 0:
        raise EvidenceError(
            f"capacity probe exited {process.returncode}: {stderr.strip()}"
        )
    if peak_rss_bytes <= 0:
        raise EvidenceError("capacity probe produced no resident-memory sample")
    try:
        result = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise EvidenceError(f"capacity probe emitted invalid JSON: {error}") from error
    if not isinstance(result, dict):
        raise EvidenceError("capacity probe output must be a JSON object")
    return result, peak_rss_bytes, rss_source


def _probe_arguments(
    *,
    scenario: str,
    candidate: tuple[int, int, int],
    messages: int,
    ingress_payload_bytes: int,
    egress_payload_bytes: int,
    observer_stall_us: int,
    retry_backoff_us: int,
    sampling_hold_us: int,
) -> list[str]:
    """Construct one fully explicit native-probe argument vector."""

    ingress, egress, turn = candidate
    stall = observer_stall_us if scenario == "observer-stall" else 0
    return [
        "--scenario",
        scenario,
        "--ingress-capacity",
        str(ingress),
        "--egress-capacity",
        str(egress),
        "--turn-budget",
        str(turn),
        "--messages",
        str(messages),
        "--ingress-payload-bytes",
        str(ingress_payload_bytes),
        "--egress-payload-bytes",
        str(egress_payload_bytes),
        "--observer-stall-us",
        str(stall),
        "--retry-backoff-us",
        str(retry_backoff_us),
        "--sampling-hold-us",
        str(sampling_hold_us),
    ]


def _rust_host_triple() -> str:
    """Read the executing Rust toolchain host triple."""

    output = _run(["rustc", "-vV"])
    assert isinstance(output, str)
    match = re.search(r"^host: (.+)$", output, re.MULTILINE)
    if not match:
        raise EvidenceError("rustc -vV omitted the host triple")
    return match.group(1)


def _total_memory_bytes() -> int:
    """Return physical memory for the executing measurement host."""

    if sys.platform.startswith("linux"):
        text = Path("/proc/meminfo").read_text(encoding="utf-8")
        match = re.search(r"^MemTotal:\s+(\d+)\s+kB$", text, re.MULTILINE)
        if match:
            return int(match.group(1)) * 1024
    elif sys.platform == "darwin":
        output = subprocess.check_output(["sysctl", "-n", "hw.memsize"], text=True)
        return int(output.strip())
    raise EvidenceError("could not determine physical memory")


def _cpu_model() -> str:
    """Return a stable human-readable CPU model when the host exposes one."""

    if sys.platform.startswith("linux"):
        text = Path("/proc/cpuinfo").read_text(encoding="utf-8")
        for key in ("model name", "model", "Hardware"):
            match = re.search(rf"^{re.escape(key)}\s*:\s*(.+)$", text, re.MULTILINE)
            if match:
                return match.group(1).strip()
    elif sys.platform == "darwin":
        for key in ("machdep.cpu.brand_string", "hw.model"):
            completed = subprocess.run(
                ["sysctl", "-n", key], check=False, capture_output=True, text=True
            )
            if completed.returncode == 0 and completed.stdout.strip():
                return completed.stdout.strip()
    return platform.processor() or "unknown"


def _median(values: Iterable[int]) -> int:
    """Return an integer median for an odd-sized evidence sample."""

    return int(statistics.median(values))


def _summaries(runs: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Aggregate repeated runs without discarding their raw records."""

    summaries = []
    keys = sorted({(run["candidate_id"], run["probe"]["config"]["scenario"]) for run in runs})
    for candidate_id, scenario in keys:
        selected = [
            run
            for run in runs
            if run["candidate_id"] == candidate_id
            and run["probe"]["config"]["scenario"] == scenario
        ]
        summaries.append(
            {
                "candidate_id": candidate_id,
                "scenario": scenario,
                "samples": len(selected),
                "median_peak_rss_bytes": _median(
                    run["peak_rss_bytes"] for run in selected
                ),
                "median_probe_duration_ns": _median(
                    run["probe"]["probe_duration_ns"] for run in selected
                ),
                "median_peak_owned_envelope_bytes": _median(
                    run["probe"]["peak_owned_envelope_bytes"] for run in selected
                ),
                "median_service_p95_ns": _median(
                    run["probe"]["service_latency"]["p95_ns"] for run in selected
                ),
                "median_delivery_p95_ns": _median(
                    run["probe"]["delivery_latency"]["p95_ns"] for run in selected
                ),
                "maximum_ingress_full_observations": max(
                    run["probe"]["ingress_full_observations"] for run in selected
                ),
                "maximum_egress_backpressured_records": max(
                    run["probe"]["egress_backpressured_records"] for run in selected
                ),
                "all_sequence_clean": all(
                    run["probe"]["sequence_errors"] == 0 for run in selected
                ),
                "all_envelope_bytes_released": all(
                    run["probe"]["owned_envelope_bytes_at_end"] == 0
                    for run in selected
                ),
            }
        )
    return summaries


def _validate_distribution(distribution: dict[str, Any], samples: int) -> None:
    """Validate monotonic latency quantiles and their sample count."""

    if distribution.get("samples") != samples:
        raise EvidenceError("latency sample count does not match accepted requests")
    values = [
        distribution.get("minimum_ns"),
        distribution.get("p50_ns"),
        distribution.get("p95_ns"),
        distribution.get("p99_ns"),
        distribution.get("maximum_ns"),
    ]
    if any(not isinstance(value, int) or value < 0 for value in values):
        raise EvidenceError("latency quantiles must be nonnegative integers")
    if values != sorted(values):
        raise EvidenceError("latency quantiles are not monotonic")


def _validate_probe(probe: dict[str, Any]) -> None:
    """Validate one raw native-probe result semantically."""

    version = probe.get("schema_version")
    expected_workloads = {
        "CPY-CAPACITY-PROBE-1": "bounded-crossbeam-transport-with-empty-endpoint-safe-turn",
        "CPY-CAPACITY-PROBE-2": "bounded-native-service-with-empty-endpoint-safe-turn-and-os-readiness",
    }
    if version not in expected_workloads:
        raise EvidenceError("unexpected native probe schema")
    if probe.get("workload") != expected_workloads[version]:
        raise EvidenceError("native probe workload does not match its schema version")
    config = probe.get("config")
    if not isinstance(config, dict) or config.get("scenario") not in SCENARIOS:
        raise EvidenceError("native probe scenario is invalid")
    accepted = probe.get("accepted_requests")
    completed = probe.get("completed_records")
    if not isinstance(accepted, int) or accepted <= 0 or completed != accepted:
        raise EvidenceError("every accepted request must produce one completed record")
    if probe.get("sequence_errors") != 0:
        raise EvidenceError("native probe record sequence is not exact")
    if probe.get("peak_ingress_depth", 0) > config.get("ingress_capacity", -1):
        raise EvidenceError("native probe exceeded ingress capacity")
    if probe.get("peak_egress_depth", 0) > config.get("egress_capacity", -1):
        raise EvidenceError("native probe exceeded egress capacity")
    if probe.get("owned_envelope_bytes_at_end") != 0:
        raise EvidenceError("native probe retained owned envelope bytes")
    offered = probe.get("offered_requests")
    terminal_rejections = probe.get("terminal_admission_rejections")
    if config["scenario"] == "cold-burst":
        if accepted != min(offered, config["ingress_capacity"]):
            raise EvidenceError("cold burst did not fill exactly to ingress capacity")
        if terminal_rejections != offered - accepted:
            raise EvidenceError("cold-burst terminal rejection accounting is inconsistent")
    elif accepted != offered or terminal_rejections != 0:
        raise EvidenceError("retrying scenarios must eventually accept every offered request")
    _validate_distribution(probe.get("service_latency", {}), accepted)
    _validate_distribution(probe.get("delivery_latency", {}), accepted)


def validate_bundle_value(bundle: dict[str, Any]) -> None:
    """Validate one in-memory CPY capacity evidence bundle."""

    if bundle.get("schema_version") != "CPY-CAPACITY-EVIDENCE-1":
        raise EvidenceError("unexpected capacity evidence schema")
    if bundle.get("normative_decision") is not False:
        raise EvidenceError("measurement bundles cannot select normative capacities")
    method = bundle.get("method")
    if not isinstance(method, dict):
        raise EvidenceError("capacity evidence method is missing")
    candidates = method.get("candidates")
    iterations = method.get("iterations")
    if not isinstance(candidates, list) or not candidates:
        raise EvidenceError("capacity evidence has no candidates")
    if not isinstance(iterations, int) or iterations < 3 or iterations % 2 == 0:
        raise EvidenceError("capacity evidence iterations must be odd and at least three")
    if method.get("scenarios") != list(SCENARIOS):
        raise EvidenceError("capacity evidence scenario set or order changed")
    candidate_ids = {candidate.get("candidate_id") for candidate in candidates}
    if len(candidate_ids) != len(candidates) or None in candidate_ids:
        raise EvidenceError("capacity candidate identifiers must be unique")
    runs = bundle.get("runs")
    if not isinstance(runs, list):
        raise EvidenceError("capacity evidence runs are missing")
    expected = {
        (candidate_id, scenario, iteration)
        for candidate_id in candidate_ids
        for scenario in SCENARIOS
        for iteration in range(1, iterations + 1)
    }
    actual = set()
    rss_sources = set()
    for run in runs:
        key = (
            run.get("candidate_id"),
            run.get("probe", {}).get("config", {}).get("scenario"),
            run.get("iteration"),
        )
        if key in actual:
            raise EvidenceError(f"duplicate capacity evidence run: {key}")
        actual.add(key)
        if not isinstance(run.get("peak_rss_bytes"), int) or run["peak_rss_bytes"] <= 0:
            raise EvidenceError("capacity evidence run has no peak RSS")
        rss_sources.add(run.get("rss_source"))
        _validate_probe(run.get("probe", {}))
    if actual != expected:
        raise EvidenceError("capacity evidence does not cover the complete candidate matrix")
    if len(rss_sources) != 1 or None in rss_sources:
        raise EvidenceError("capacity evidence mixed or omitted RSS samplers")
    summaries = bundle.get("summaries")
    expected_summary_keys = {
        (candidate_id, scenario) for candidate_id in candidate_ids for scenario in SCENARIOS
    }
    actual_summary_keys = {
        (summary.get("candidate_id"), summary.get("scenario"))
        for summary in summaries or []
    }
    if actual_summary_keys != expected_summary_keys:
        raise EvidenceError("capacity evidence summaries do not cover the matrix")
    if any(
        summary.get("samples") != iterations
        or summary.get("all_sequence_clean") is not True
        or summary.get("all_envelope_bytes_released") is not True
        for summary in summaries
    ):
        raise EvidenceError("capacity evidence summary acceptance flags are incomplete")


def _validate_schema(bundle: dict[str, Any]) -> None:
    """Validate the formal JSON Schema when jsonschema is installed."""

    try:
        import jsonschema
    except ImportError:
        return
    schema = _load_json(SCHEMA_PATH)
    try:
        jsonschema.Draft202012Validator(schema).validate(bundle)
    except jsonschema.ValidationError as error:
        raise EvidenceError(f"capacity evidence schema violation: {error.message}") from error


def validate_bundle(path: Path, *, verify_sources: bool = True) -> dict[str, Any]:
    """Validate a retained bundle and optionally its immutable source records."""

    bundle = _load_json(path)
    _validate_schema(bundle)
    validate_bundle_value(bundle)
    if verify_sources:
        commit = bundle["source"]["commit"]
        _git("cat-file", "-e", f"{commit}^{{commit}}")
        expected = _source_records(commit)
        if bundle["source"]["probe_sources"] != expected:
            raise EvidenceError("capacity evidence source hashes do not match its commit")
        lock_record = bundle["source"]["cargo_lock"]
        lock_path = REPO_ROOT / lock_record["path"]
        if not lock_path.is_file() or _sha256(lock_path.read_bytes()) != lock_record["sha256"]:
            raise EvidenceError("retained capacity Cargo.lock is missing or changed")
    return bundle


def _write_new(path: Path, data: bytes) -> None:
    """Create a new evidence artifact without overwriting retained history."""

    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("xb") as handle:
            handle.write(data)
    except FileExistsError as error:
        raise EvidenceError(f"refusing to overwrite retained evidence: {path}") from error


def capture(arguments: argparse.Namespace) -> Path:
    """Capture one clean-source host or physical-board candidate matrix."""

    if arguments.profile == "embedded-linux-direct" and not arguments.physical_board:
        raise EvidenceError("embedded-linux-direct capture requires --physical-board")
    commit = _require_clean_source()
    lock_path = REPO_ROOT / "Cargo.lock"
    if not lock_path.is_file():
        raise EvidenceError("Cargo.lock is required for capacity capture")
    lock_bytes = lock_path.read_bytes()
    metadata = _cargo_metadata()
    binary = _build_probe(metadata)
    binary_bytes = binary.read_bytes()
    candidates = arguments.candidate or list(DEFAULT_CANDIDATES)
    candidate_records = [
        {
            "candidate_id": _candidate_id(candidate),
            "ingress_capacity": candidate[0],
            "egress_capacity": candidate[1],
            "turn_budget": candidate[2],
        }
        for candidate in candidates
    ]
    runs = []
    rss_source = None

    for candidate in candidates:
        for scenario in SCENARIOS:
            probe_arguments = _probe_arguments(
                scenario=scenario,
                candidate=candidate,
                messages=arguments.messages,
                ingress_payload_bytes=arguments.ingress_payload_bytes,
                egress_payload_bytes=arguments.egress_payload_bytes,
                observer_stall_us=arguments.observer_stall_us,
                retry_backoff_us=arguments.retry_backoff_us,
                sampling_hold_us=arguments.sampling_hold_us,
            )
            print(
                f"warmup {_candidate_id(candidate)} {scenario}",
                file=sys.stderr,
                flush=True,
            )
            for _ in range(arguments.warmups):
                _run_probe(binary, probe_arguments)
            for iteration in range(1, arguments.iterations + 1):
                print(
                    f"measure {_candidate_id(candidate)} {scenario} {iteration}/{arguments.iterations}",
                    file=sys.stderr,
                    flush=True,
                )
                probe, peak_rss_bytes, measured_rss_source = _run_probe(
                    binary, probe_arguments
                )
                if rss_source is not None and measured_rss_source != rss_source:
                    raise EvidenceError("RSS sampler changed during capacity capture")
                rss_source = measured_rss_source
                runs.append(
                    {
                        "candidate_id": _candidate_id(candidate),
                        "iteration": iteration,
                        "peak_rss_bytes": peak_rss_bytes,
                        "rss_source": measured_rss_source,
                        "probe": probe,
                    }
                )

    short_commit = commit[:8]
    retained_lock = GENERATED_DIR / f"CPY-CAPACITY-CARGO-LOCK-{short_commit}.lock"
    qualification = (
        "physical-board-candidate"
        if arguments.profile == "embedded-linux-direct"
        else "diagnostic-host"
    )
    bundle = {
        "schema_version": "CPY-CAPACITY-EVIDENCE-1",
        "evidence_id": f"CPY-CAPACITY-{arguments.profile.upper()}-{short_commit}",
        "created_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "qualification": qualification,
        "profile": arguments.profile,
        "normative_decision": False,
        "source": {
            "commit": commit,
            "tree_clean": True,
            "probe_sources": _source_records(commit),
            "cargo_lock": {
                "path": retained_lock.relative_to(REPO_ROOT).as_posix(),
                "sha256": _sha256(lock_bytes),
            },
        },
        "toolchain": {
            "rustc_version": _run(["rustc", "--version"]).strip(),
            "cargo_version": _run(["cargo", "--version"]).strip(),
            "python_version": platform.python_version(),
            "crossbeam_channel": _crossbeam_pin(lock_bytes),
            "probe_binary_sha256": _sha256(binary_bytes),
            "build_profile": "release",
        },
        "environment": {
            "hardware_label": arguments.hardware_label,
            "operating_system": platform.platform(),
            "kernel": platform.release(),
            "machine": platform.machine(),
            "cpu_model": _cpu_model(),
            "rust_host_triple": _rust_host_triple(),
            "logical_cpus": os.cpu_count() or 1,
            "total_memory_bytes": _total_memory_bytes(),
            "rss_sampler": rss_source,
            "physical_board": bool(arguments.physical_board),
        },
        "method": {
            "clock": "std::time::Instant monotonic clock",
            "warmups": arguments.warmups,
            "iterations": arguments.iterations,
            "messages": arguments.messages,
            "ingress_payload_bytes": arguments.ingress_payload_bytes,
            "egress_payload_bytes": arguments.egress_payload_bytes,
            "observer_stall_us": arguments.observer_stall_us,
            "retry_backoff_us": arguments.retry_backoff_us,
            "sampling_hold_us": arguments.sampling_hold_us,
            "scenarios": list(SCENARIOS),
            "candidates": candidate_records,
            "limitations": [
                "Native-service workload executes an empty neutral Endpoint Safe Turn; it does not measure actor, render, frame, input, Python, or PyO3 work.",
                "Owned-envelope bytes exclude native service, channel, and readiness allocation; peak RSS separately measures the whole native process.",
                "Readiness values count successful producer-side coalesced eventfd or self-pipe notifications, not consumer wakeups.",
                "Candidate values are evidence inputs and this bundle makes no default, maximum, budget, or ratification decision.",
                "Host results cannot satisfy the CPY-01 embedded-Linux reference-board requirement.",
            ],
        },
        "runs": runs,
        "summaries": _summaries(runs),
    }
    _validate_schema(bundle)
    validate_bundle_value(bundle)
    output = arguments.output.resolve()
    _write_new(retained_lock, lock_bytes)
    _write_new(
        output,
        (json.dumps(bundle, indent=2, sort_keys=True) + "\n").encode("utf-8"),
    )
    validate_bundle(output)
    return output


def _parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""

    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    capture_parser = subparsers.add_parser("capture", help="capture a clean-source matrix")
    capture_parser.add_argument(
        "--output", type=Path, default=DEFAULT_OUTPUT, help="new evidence bundle path"
    )
    capture_parser.add_argument(
        "--profile",
        choices=("host-headless", "embedded-linux-direct"),
        default="host-headless",
    )
    capture_parser.add_argument("--hardware-label", required=True)
    capture_parser.add_argument("--physical-board", action="store_true")
    capture_parser.add_argument(
        "--candidate",
        action="append",
        type=_parse_candidate,
        help="repeat ingress:egress:turn candidate; default matrix is 8/16/4 through 64/128/32",
    )
    capture_parser.add_argument("--warmups", type=int, default=1)
    capture_parser.add_argument("--iterations", type=int, default=5)
    capture_parser.add_argument("--messages", type=int, default=1024)
    capture_parser.add_argument("--ingress-payload-bytes", type=int, default=256)
    capture_parser.add_argument("--egress-payload-bytes", type=int, default=128)
    capture_parser.add_argument("--observer-stall-us", type=int, default=50_000)
    capture_parser.add_argument("--retry-backoff-us", type=int, default=50)
    capture_parser.add_argument("--sampling-hold-us", type=int, default=20_000)

    validate_parser = subparsers.add_parser("validate", help="validate retained evidence")
    validate_parser.add_argument("path", type=Path)
    return parser


def main() -> int:
    """Run capture or validation and report a concise result."""

    parser = _parser()
    arguments = parser.parse_args()
    try:
        if arguments.command == "capture":
            for name in (
                "warmups",
                "iterations",
                "messages",
                "ingress_payload_bytes",
                "egress_payload_bytes",
                "observer_stall_us",
                "retry_backoff_us",
                "sampling_hold_us",
            ):
                value = getattr(arguments, name)
                minimum = 0 if name in {"observer_stall_us", "retry_backoff_us"} else 1
                if value < minimum:
                    raise EvidenceError(f"--{name.replace('_', '-')} must be at least {minimum}")
            if arguments.iterations < 3 or arguments.iterations % 2 == 0:
                raise EvidenceError("--iterations must be odd and at least three")
            path = capture(arguments)
            print(path.relative_to(REPO_ROOT))
        else:
            bundle = validate_bundle(arguments.path.resolve())
            print(
                f"capacity evidence valid: {len(bundle['runs'])} runs, "
                f"{len(bundle['summaries'])} summaries"
            )
    except (EvidenceError, subprocess.CalledProcessError) as error:
        print(f"cpy_capacity_probe: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
