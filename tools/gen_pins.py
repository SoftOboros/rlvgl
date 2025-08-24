#!/usr/bin/env python3
"""
Generate aggregated board database files from vendor sources.

This tool scans an input directory for JSON files describing boards and
writes a combined ``boards.json`` file to the output directory. The
script is a placeholder for future per-vendor converters.

Usage::
    python tools/gen_pins.py --input vendor_dir --output out_dir
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
from typing import List


def gather_boards(input_dir: pathlib.Path) -> List[dict]:
    """Read all ``board_*.json`` files under ``input_dir`` and return their data."""
    boards = []
    for path in input_dir.glob("board_*.json"):
        with path.open("r", encoding="utf-8") as src:
            boards.append(json.load(src))
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
    if mcu_src.exists():
        shutil.copy(mcu_src, args.output / "mcu.json")


if __name__ == "__main__":
    main()
