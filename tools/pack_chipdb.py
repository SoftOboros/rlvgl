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
import shutil
import subprocess
import tempfile

try:
    import zstandard as zstd
except ModuleNotFoundError:
    zstd = None


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


def compress_blob(blob: bytes) -> bytes:
    """Return a zstd-compressed chip database blob."""
    if zstd is not None:
        cctx = zstd.ZstdCompressor(level=19)
        return cctx.compress(blob)

    zstd_bin = shutil.which("zstd")
    if zstd_bin is None:
        raise RuntimeError(
            "pack_chipdb.py requires either the Python 'zstandard' module or the 'zstd' CLI"
        )

    with tempfile.TemporaryDirectory() as tmp_dir:
        src = pathlib.Path(tmp_dir) / "chipdb.bin"
        dst = pathlib.Path(tmp_dir) / "chipdb.bin.zst"
        src.write_bytes(blob)
        subprocess.run(
            [zstd_bin, "-19", "-f", str(src), "-o", str(dst)],
            check=True,
        )
        return dst.read_bytes()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()

    blob = build_blob(args.input)
    with args.output.open("wb") as dst:
        dst.write(compress_blob(blob))


if __name__ == "__main__":
    main()
