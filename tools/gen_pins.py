#!/usr/bin/env python3
"""
Extract MCU definition files from vendor sources.

This tool copies an ``mcu.json`` file from the input directory (or its
parent) to the output directory. It is a placeholder for future
per-vendor converters.

Usage::
    python tools/gen_pins.py --input vendor_dir --output out_dir
"""

from __future__ import annotations

import argparse
import pathlib
import shutil


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()

    args.output.mkdir(parents=True, exist_ok=True)

    mcu_src = args.input / "mcu.json"
    if not mcu_src.exists():
        mcu_src = args.input.parent / "mcu.json"
    if mcu_src.exists():
        shutil.copy(mcu_src, args.output / "mcu.json")


if __name__ == "__main__":
    main()
