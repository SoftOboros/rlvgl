#!/usr/bin/env python3
"""
Generate aggregated board database files from vendor sources.

This tool scans an input directory for JSON files describing boards and
writes a combined ``boards.json`` file to the output directory. If an
``mcu.json`` file is present alongside the boards data (or in the parent
directory) it is copied verbatim. The script is a placeholder for future
per-vendor converters.

Usage::
    python tools/gen_pins.py --input vendor_dir --output out_dir
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
from typing import Dict


def gather_boards(input_dir: pathlib.Path) -> Dict[str, dict]:
    """Read all ``*.json`` files under ``input_dir`` except ``mcu.json`` and return a mapping by board name."""
    boards = {}
    for path in input_dir.glob("*.json"):
        if path.name == "mcu.json":
            continue
        with path.open("r", encoding="utf-8") as src:
            data = json.load(src)
        if "chip" not in data:
            continue
        name = data.pop("board", path.stem)
        boards[name] = data
    return boards


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()

    args.output.mkdir(parents=True, exist_ok=True)

    boards_src = args.input / "boards.json"
    if boards_src.exists():
        shutil.copy(boards_src, args.output / "boards.json")
    else:
        boards = gather_boards(args.input)
        with (args.output / "boards.json").open("w", encoding="utf-8") as dst:
            json.dump({"boards": boards}, dst, indent=2)

    mcu_src = args.input / "mcu.json"
    if not mcu_src.exists():
        mcu_src = args.input.parent / "mcu.json"
    if mcu_src.exists():
        shutil.copy(mcu_src, args.output / "mcu.json")


if __name__ == "__main__":
    main()
