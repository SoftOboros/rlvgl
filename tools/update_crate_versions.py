#!/usr/bin/env python3
"""update_crate_versions.py - Bump internal crate versions based on git changes."""

from __future__ import annotations

import argparse
import collections
import pathlib
import re
import subprocess
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent


def find_crates() -> dict[str, pathlib.Path]:
    """Locate Cargo manifests and return a map of crate name to manifest path."""
    crates: dict[str, pathlib.Path] = {}
    for path in ROOT.glob("**/Cargo.toml"):
        try:
            data = tomllib.loads(path.read_text(encoding="utf-8"))
        except Exception:  # pragma: no cover - invalid toml
            continue
        package = data.get("package")
        if package:
            crates[package["name"]] = path
    return crates


def build_dependents(crates: dict[str, pathlib.Path]) -> dict[str, set[str]]:
    """Return reverse dependency mapping for workspace crates."""
    dependents: dict[str, set[str]] = {name: set() for name in crates}
    for name, manifest in crates.items():
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        for dep in data.get("dependencies", {}):
            if dep in crates:
                dependents[dep].add(name)
    return dependents


def latest_tag() -> str:
    """Get the latest v* tag from git."""
    return subprocess.check_output(
        ["git", "describe", "--tags", "--abbrev=0", "--match", "v*"],
        cwd=ROOT,
        text=True,
    ).strip()


def changed_crates(
    crates: dict[str, pathlib.Path], dependents: dict[str, set[str]], tag: str
) -> set[str]:
    """Determine which crates changed since ``tag`` and propagate to dependents."""
    output = subprocess.check_output(
        ["git", "diff", "--name-only", tag], cwd=ROOT, text=True
    )
    changed: set[str] = set()
    for line in output.splitlines():
        p = ROOT / line
        for name, manifest in sorted(crates.items(), key=lambda x: len(x[1].parent.parts), reverse=True):
            if p.is_relative_to(manifest.parent):
                changed.add(name)
                break
    queue = collections.deque(changed)
    while queue:
        crate = queue.popleft()
        for dep in dependents.get(crate, set()):
            if dep not in changed:
                changed.add(dep)
                queue.append(dep)
    return changed


def bump_version(version: str, roll: int | None) -> str:
    """Return ``version`` bumped according to ``roll``."""
    major, minor, patch = map(int, version.split("."))
    if roll == 2:
        major += 1
        minor = patch = 0
    elif roll == 1:
        minor += 1
        patch = 0
    else:
        patch += 1
    return f"{major}.{minor}.{patch}"


def update_versions(
    crates: dict[str, pathlib.Path],
    changed: set[str],
    roll: int | None,
    dry_run: bool,
) -> dict[str, str]:
    """Apply version bumps and update internal dependency constraints."""
    new_versions: dict[str, str] = {}
    for name in changed:
        text = crates[name].read_text(encoding="utf-8")
        match = re.search(r'^version\s*=\s*"(\d+\.\d+\.\d+)"', text, re.MULTILINE)
        if match:
            new_versions[name] = bump_version(match.group(1), roll)
    if roll:
        highest = max(tuple(map(int, v.split("."))) for v in new_versions.values())
        high_str = ".".join(map(str, highest))
        for name in new_versions:
            new_versions[name] = high_str
    for name, manifest in crates.items():
        text = manifest.read_text(encoding="utf-8")
        if name in new_versions:
            text = re.sub(
                r'^(version\s*=\s*)"\d+\.\d+\.\d+"',
                rf'\1"{new_versions[name]}"',
                text,
                count=1,
                flags=re.MULTILINE,
            )
        for dep, ver in new_versions.items():
            if dep == name:
                continue
            pattern1 = rf'({re.escape(dep)}\s*=\s*{{[^}}]*?version\s*=\s*")\d+\.\d+\.\d+("[^}}]*}})'
            text = re.sub(pattern1, lambda m: f'{m.group(1)}{ver}{m.group(2)}', text, flags=re.MULTILINE)
            pattern2 = rf'(\[dependencies\.{re.escape(dep)}\][^\[]*?version\s*=\s*")\d+\.\d+\.\d+("[^\n]*")'
            text = re.sub(pattern2, lambda m: f'{m.group(1)}{ver}{m.group(2)}', text, flags=re.MULTILINE)
        if not dry_run:
            manifest.write_text(text, encoding="utf-8")
    return new_versions


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Bump crate versions based on changes since last v* tag."
    )
    parser.add_argument(
        "--roll", type=int, choices=[1, 2], help="Roll minor (1) or major (2) version"
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="Report new versions without editing files"
    )
    args = parser.parse_args()

    crates = find_crates()
    dependents = build_dependents(crates)
    tag = latest_tag()
    changed = changed_crates(crates, dependents, tag)
    if not changed:
        print("no crates changed")
        return
    new_versions = update_versions(crates, changed, args.roll, args.dry_run)
    for name, ver in sorted(new_versions.items()):
        print(f"{name} -> {ver}")


if __name__ == "__main__":
    main()
