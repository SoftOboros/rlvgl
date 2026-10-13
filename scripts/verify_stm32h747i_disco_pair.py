#!/usr/bin/env python3
"""Verify the diagnostic STM32H747I-DISCO CM7/CM4 image pair.

This is prerequisite evidence for MPY-08, not a transport qualification. It
checks the existing paired-image build and legacy mailbox candidate without
selecting MPY shared-memory geometry, cache policy, signaling, or wire format.
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
from typing import Any


CM7_FLASH = (0x0800_0000, 0x0810_0000)
CM4_FLASH = (0x0810_0000, 0x0820_0000)
LEGACY_MAILBOX = (0x3004_7000, 0x3004_7400)


class VerificationError(RuntimeError):
    """Raised when a paired-image invariant is not satisfied."""


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


def require(condition: bool, message: str, checks: list[dict[str, Any]]) -> None:
    """Record a passing check or raise a diagnostic failure."""

    if not condition:
        raise VerificationError(message)
    checks.append({"name": message, "passed": True})


def cpu_name(elf: pathlib.Path) -> str:
    """Read the ELF's Arm CPU name from build attributes."""

    output = run("rust-readobj", "--arch-specific", str(elf))
    match = re.search(r"TagName:\s*CPU_name\s*\n\s*Value:\s*([^\s]+)", output)
    if match is None:
        # rust-readobj has used both field orders across LLVM releases.
        match = re.search(r"TagName:\s*CPU_name.*?Value:\s*([^\s}]+)", output, re.S)
    if match is None:
        raise VerificationError(f"cannot read CPU_name from {elf}")
    return match.group(1)


def program_headers(elf: pathlib.Path) -> tuple[int, list[dict[str, int]]]:
    """Return the entry point and LOAD program headers from an ELF."""

    output = run("rust-readobj", "--elf-output-style=GNU", "--program-headers", str(elf))
    entry_match = re.search(r"Entry point\s+0x([0-9a-fA-F]+)", output)
    if entry_match is None:
        raise VerificationError(f"cannot read entry point from {elf}")

    segments: list[dict[str, int]] = []
    load_pattern = re.compile(
        r"^\s*LOAD\s+"
        r"0x([0-9a-fA-F]+)\s+"
        r"0x([0-9a-fA-F]+)\s+"
        r"0x([0-9a-fA-F]+)\s+"
        r"0x([0-9a-fA-F]+)\s+"
        r"0x([0-9a-fA-F]+)",
        re.MULTILINE,
    )
    for match in load_pattern.finditer(output):
        offset, virtual, physical, file_size, memory_size = (
            int(value, 16) for value in match.groups()
        )
        segments.append(
            {
                "offset": offset,
                "virtual_address": virtual,
                "physical_address": physical,
                "file_size": file_size,
                "memory_size": memory_size,
            }
        )
    if not segments:
        raise VerificationError(f"cannot read LOAD segments from {elf}")
    return int(entry_match.group(1), 16), segments


def symbols(elf: pathlib.Path, names: set[str]) -> dict[str, int]:
    """Read selected absolute or defined symbols from an ELF."""

    found: dict[str, int] = {}
    for line in run("rust-nm", "-P", "--defined-only", str(elf)).splitlines():
        fields = line.split()
        if len(fields) >= 3 and fields[0] in names:
            found[fields[0]] = int(fields[2], 16)
    missing = names.difference(found)
    if missing:
        raise VerificationError(f"{elf} lacks symbols: {', '.join(sorted(missing))}")
    return found


def relative(path: pathlib.Path, root: pathlib.Path) -> str:
    """Return a repository-relative path where possible."""

    try:
        return str(path.resolve().relative_to(root.resolve()))
    except ValueError:
        return str(path.resolve())


def version(*args: str) -> str:
    """Return a one-line tool version."""

    return run(*args).splitlines()[0]


def llvm_version(*args: str) -> str:
    """Return LLVM's concrete version line rather than its banner."""

    lines = (line.strip() for line in run(*args).splitlines())
    return next((line for line in lines if line.startswith("LLVM version")), "unknown")


def image_record(
    *,
    core: str,
    elf: pathlib.Path,
    expected_cpu: str,
    flash: tuple[int, int],
    checks: list[dict[str, Any]],
) -> dict[str, Any]:
    """Validate and describe one core image."""

    actual_cpu = cpu_name(elf)
    if actual_cpu != expected_cpu:
        raise VerificationError(
            f"{core} CPU_name is {actual_cpu}, expected {expected_cpu}"
        )
    checks.append({"name": f"{core} CPU_name is {expected_cpu}", "passed": True})

    entry, segments = program_headers(elf)
    executable_entry = entry & ~1
    require(
        flash[0] <= executable_entry < flash[1],
        f"{core} entry point is in its flash bank",
        checks,
    )

    flash_segments = [
        segment
        for segment in segments
        if segment["file_size"] > 0
        and flash[0] <= segment["physical_address"] < flash[1]
    ]
    require(bool(flash_segments), f"{core} has file-backed flash segments", checks)
    require(
        all(
            segment["physical_address"] + segment["file_size"] <= flash[1]
            for segment in flash_segments
        ),
        f"{core} file-backed flash segments fit their bank",
        checks,
    )
    require(
        all(
            not (
                segment["file_size"] > 0
                and segment["physical_address"] < LEGACY_MAILBOX[1]
                and segment["physical_address"] + segment["file_size"]
                > LEGACY_MAILBOX[0]
            )
            for segment in segments
        ),
        f"{core} load segments do not overlap the legacy mailbox candidate",
        checks,
    )

    mailbox = symbols(elf, {"_mailbox_base", "_mailbox_size"})
    require(
        mailbox["_mailbox_base"] == LEGACY_MAILBOX[0],
        f"{core} exports the legacy mailbox base",
        checks,
    )
    require(
        mailbox["_mailbox_size"] == LEGACY_MAILBOX[1] - LEGACY_MAILBOX[0],
        f"{core} exports the legacy mailbox extent",
        checks,
    )

    return {
        "core": core,
        "path": str(elf),
        "sha256": sha256(elf),
        "size_bytes": elf.stat().st_size,
        "cpu_name": actual_cpu,
        "entry_point": entry,
        "flash_bank": {"base": flash[0], "end_exclusive": flash[1]},
        "load_segments": segments,
        "mailbox_symbols": mailbox,
    }


def main() -> int:
    """Verify the pair and write the requested diagnostic evidence."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cm7", required=True, type=pathlib.Path)
    parser.add_argument("--cm4", required=True, type=pathlib.Path)
    parser.add_argument("--json-out", type=pathlib.Path)
    args = parser.parse_args()

    root = pathlib.Path(run("git", "rev-parse", "--show-toplevel"))
    cm7 = args.cm7.resolve()
    cm4 = args.cm4.resolve()
    checks: list[dict[str, Any]] = []

    require(cm7.is_file(), "CM7 ELF exists", checks)
    require(cm4.is_file(), "CM4 ELF exists", checks)

    cm7_record = image_record(
        core="cm7",
        elf=cm7,
        expected_cpu="cortex-m7",
        flash=CM7_FLASH,
        checks=checks,
    )
    cm4_record = image_record(
        core="cm4",
        elf=cm4,
        expected_cpu="cortex-m4",
        flash=CM4_FLASH,
        checks=checks,
    )
    require(
        cm7_record["mailbox_symbols"] == cm4_record["mailbox_symbols"],
        "paired images export identical mailbox symbols",
        checks,
    )

    cm7_bin = cm7.with_suffix(cm7.suffix + ".bin")
    cm7_hex = cm7.with_suffix(cm7.suffix + ".hex")
    cm4_hex = cm4.with_suffix(cm4.suffix + ".hex")
    for artifact, label in (
        (cm7_bin, "CM7 binary"),
        (cm7_hex, "CM7 Intel HEX"),
        (cm4_hex, "CM4 Intel HEX"),
    ):
        require(artifact.is_file(), f"{label} exists", checks)
    require(
        cm7_bin.stat().st_size <= CM7_FLASH[1] - CM7_FLASH[0],
        "CM7 raw binary fits flash bank 1 without a RAM-address gap",
        checks,
    )

    commit = run("git", "rev-parse", "HEAD", cwd=root)
    tree_clean = not run(
        "git", "status", "--porcelain", "--untracked-files=all", cwd=root
    )
    cargo_lock = root / "Cargo.lock"
    source_paths = [
        root / "Makefile",
        root / "examples/stm32h747i-disco/memory.x",
        root / "examples/stm32h747i-disco/memory_cm4.x",
        root / "examples/stm32h747i-disco/src/ipc.rs",
        root / "examples/stm32h747i-disco/src/main.rs",
        root / "examples/stm32h747i-disco/src/cm4_main.rs",
        root / "scripts/verify_stm32h747i_disco_pair.py",
    ]

    now = dt.datetime.now(dt.timezone.utc)
    evidence: dict[str, Any] = {
        "schema_version": "mpy08-disco-pair-diagnostic-v1",
        "evidence_id": f"MPY08-DISCO-PAIR-{commit[:12]}-{now.date().isoformat()}",
        "created_at": now.isoformat().replace("+00:00", "Z"),
        "qualification": "diagnostic_prerequisite",
        "profile": "stm32h747i-disco-legacy-mailbox-candidate",
        "normative_decision": False,
        "source": {
            "commit": commit,
            "tree_clean": tree_clean,
            "probe_sources": [
                {
                    "path": relative(path, root),
                    "sha256": sha256(path),
                }
                for path in source_paths
            ],
            "cargo_lock": {
                "path": relative(cargo_lock, root),
                "sha256": sha256(cargo_lock),
            }
            if cargo_lock.is_file()
            else None,
        },
        "toolchain": {
            "rustc_version": version("rustc", "--version"),
            "cargo_version": version("cargo", "--version"),
            "rust_readobj_version": llvm_version("rust-readobj", "--version"),
            "probe_rs_version": version("probe-rs", "--version"),
            "build_profile": "debug",
        },
        "environment": {
            "hardware_label": "STM32H747I-DISCO",
            "physical_board": False,
        },
        "method": {
            "kind": "paired_elf_static_audit",
            "limitations": [
                "No physical board was attached during this static audit.",
                "The 1 KiB D2 SRAM3 mailbox is the legacy demo candidate, not an MPY-08 decision.",
                "Cache policy, publication barriers, signaling, Boot Epoch, canonical MPY framing, and capacity remain unqualified.",
            ],
        },
        "artifacts": [
            cm7_record,
            cm4_record,
            {
                "core": "cm7",
                "format": "bin",
                "path": relative(cm7_bin, root),
                "sha256": sha256(cm7_bin),
                "size_bytes": cm7_bin.stat().st_size,
            },
            {
                "core": "cm7",
                "format": "ihex",
                "path": relative(cm7_hex, root),
                "sha256": sha256(cm7_hex),
                "size_bytes": cm7_hex.stat().st_size,
            },
            {
                "core": "cm4",
                "format": "ihex",
                "path": relative(cm4_hex, root),
                "sha256": sha256(cm4_hex),
                "size_bytes": cm4_hex.stat().st_size,
            },
        ],
        "checks": checks,
    }

    if args.json_out is not None:
        destination = args.json_out.resolve()
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")

    print(
        "PASS: STM32H747I-DISCO paired-image diagnostic "
        f"({len(checks)} checks; CM7 bin {cm7_bin.stat().st_size} bytes)"
    )
    if args.json_out is not None:
        print(f"evidence={args.json_out}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError, VerificationError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        raise SystemExit(1) from error
