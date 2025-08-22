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
from typing import List


def gather_boards(input_dir: pathlib.Path) -> List[dict]:
    """Read all ``*.json`` files under ``input_dir`` and return their data."""
    boards = []
    for path in input_dir.glob("*.json"):
        with path.open("r", encoding="utf-8") as src:
            boards.append(json.load(src))
    return boards


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()

    boards = gather_boards(args.input)
    args.output.mkdir(parents=True, exist_ok=True)
    out_path = args.output / "boards.json"
    with out_path.open("w", encoding="utf-8") as dst:
        json.dump({"boards": boards}, dst, indent=2)


if __name__ == "__main__":
    main()
