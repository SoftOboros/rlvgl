#!/usr/bin/env python3
"""
Increment vendor crate versions when pin databases change.

This script locates `Cargo.toml` manifests for chip database crates and
bumps their patch version. It can operate on a directory containing
vendor crates or on individual manifest files.

Usage:
    python tools/bump_vendor_versions.py --path chipdb
    python tools/bump_vendor_versions.py --manifest chipdb/rlvgl-chips-stm/Cargo.toml
"""

from __future__ import annotations

import argparse
import pathlib
import re
from typing import Iterable


def bump_manifest(path: pathlib.Path, dry_run: bool) -> str:
    """Increment the patch version in `path` and return the new version."""
    text = path.read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"', text, re.MULTILINE)
    if not match:
        raise RuntimeError(f"no version field in {path}")
    major, minor, patch = map(int, match.groups())
    patch += 1
    new_version = f"{major}.{minor}.{patch}"
    new_text = re.sub(
        r'^(version\s*=\s*")(\d+\.\d+\.\d+)(")',
        lambda m: f'{m.group(1)}{new_version}{m.group(3)}',
        text,
        count=1,
        flags=re.MULTILINE,
    )
    if not dry_run:
        path.write_text(new_text, encoding="utf-8")
    return new_version


def find_manifests(root: pathlib.Path) -> Iterable[pathlib.Path]:
    """Yield Cargo.toml files for vendor crates under ``root``."""
    for manifest in root.glob("rlvgl-chips-*/Cargo.toml"):
        yield manifest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--path", type=pathlib.Path, help="Directory containing vendor crates")
    parser.add_argument("--manifest", action="append", type=pathlib.Path, help="Specific Cargo.toml manifest to bump")
    parser.add_argument("--dry-run", action="store_true", help="Print new versions without modifying files")
    args = parser.parse_args()

    manifests = []
    if args.path:
        manifests.extend(find_manifests(args.path))
    if args.manifest:
        manifests.extend(args.manifest)
    if not manifests:
        parser.error("provide --path or --manifest")

    for manifest in manifests:
        new_version = bump_manifest(manifest, args.dry_run)
        print(f"{manifest}: {new_version}")


if __name__ == "__main__":
    main()
