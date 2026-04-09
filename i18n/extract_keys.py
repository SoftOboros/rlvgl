#!/usr/bin/env python3
"""Scan Rust sources for t!() calls and sync keys with locale JSONs.

Usage:
    python extract_keys.py [--src ../../] [--locales locales/]

Walks all .rs files under --src, extracts t!("key") and t!("key", ...)
invocations, then:
  1. Adds missing keys to en.json with a "TODO" placeholder.
  2. Reports keys present in JSON but unused in source (stale).
  3. Reports keys added so translators know what's new.

Does NOT modify non-default locale files — translators handle those.
"""
import argparse
import json
import re
import sys
from pathlib import Path

# Matches t!("some.key") and t!("some.key", ...) — word boundary prevents
# false matches on format!(), write!(), etc.
T_PATTERN = re.compile(r'(?<![a-zA-Z_])t!\(\s*"([^"]+)"')


def find_rs_files(root: Path):
    """Yield all .rs files under root, skipping target/ and .git/."""
    for p in root.rglob("*.rs"):
        parts = p.parts
        if "target" in parts or ".git" in parts:
            continue
        yield p


def extract_keys(root: Path) -> set[str]:
    """Return all i18n keys referenced in Rust source."""
    keys = set()
    for path in find_rs_files(root):
        text = path.read_text(errors="replace")
        for m in T_PATTERN.finditer(text):
            keys.add(m.group(1))
    return keys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--src",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="Root directory to scan for .rs files (default: repo root)",
    )
    parser.add_argument(
        "--locales",
        type=Path,
        default=Path(__file__).resolve().parent / "locales",
        help="Locales directory (default: i18n/locales/)",
    )
    args = parser.parse_args()

    en_path = args.locales / "en.json"
    if not en_path.exists():
        print(f"error: {en_path} not found", file=sys.stderr)
        sys.exit(1)

    with open(en_path) as f:
        en: dict[str, str] = json.load(f)

    source_keys = extract_keys(args.src)
    json_keys = set(en.keys())

    # Keys in source but not in JSON → add with TODO placeholder.
    new_keys = sorted(source_keys - json_keys)
    # Keys in JSON but not in source → potentially stale.
    stale_keys = sorted(json_keys - source_keys)

    if new_keys:
        print(f"Adding {len(new_keys)} new key(s) to en.json:")
        for k in new_keys:
            print(f"  + {k}")
            en[k] = f"TODO: {k}"
        # Write back sorted.
        en = dict(sorted(en.items()))
        with open(en_path, "w") as f:
            json.dump(en, f, indent=2, ensure_ascii=False)
            f.write("\n")
        print(f"Updated {en_path}")
    else:
        print("No new keys found.")

    if stale_keys:
        print(f"\n{len(stale_keys)} potentially stale key(s) (in JSON, not in source):")
        for k in stale_keys:
            print(f"  ? {k}")
        print("Review and remove manually if no longer needed.")

    if not new_keys and not stale_keys:
        print("All keys in sync.")


if __name__ == "__main__":
    main()
