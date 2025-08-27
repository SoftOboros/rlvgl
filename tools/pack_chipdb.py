#!/usr/bin/env python3
"""
pack_chipdb.py - Package JSON definitions into a zstd archive.

Reads all files in an input directory, concatenates them into a simple
text-based archive, and compresses the result with Zstandard. The output
is consumed by vendor crates at publish time and later decompressed by
`rlvgl-creator`.
"""
from __future__ import annotations

import argparse
import pathlib
import zstandard as zstd


def build_blob(src: pathlib.Path) -> bytes:
    """Return concatenated `>name\n<content>\n<` blobs for all files."""
    parts = []
    for path in sorted(src.iterdir()):
        if path.is_file():
            parts.append(f">{path.name}\n".encode())
            data = path.read_bytes()
            parts.append(data)
            if not data.endswith(b"\n"):
                parts.append(b"\n")
            parts.append(b"<\n")
    return b"".join(parts)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()

    blob = build_blob(args.input)
    cctx = zstd.ZstdCompressor(level=19)
    with args.output.open("wb") as dst:
        dst.write(cctx.compress(blob))


if __name__ == "__main__":
    main()
