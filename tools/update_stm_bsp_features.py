#!/usr/bin/env python3
"""Synchronize ``stm32-*`` feature options for the STM BSP crate.

Scans the generated BSP sources for ``#[cfg(feature = "stm32-*")]``
entries and rewrites ``Cargo.toml`` with matching feature keys. This
keeps the manifest in sync with the options referenced by the source
to avoid warnings about unknown features.
"""
from __future__ import annotations

import re
from pathlib import Path
import tomllib
import tomli_w

ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = ROOT / "chips" / "stm" / "bsps" / "src"
TOML_PATH = SRC_DIR.parent / "Cargo.toml"

FEATURE_RE = re.compile(r'feature\s*=\s*"(stm32-[^"]+)"')

def collect_features() -> list[str]:
    """Return sorted ``stm32-*`` feature names present in the BSP sources."""
    features: set[str] = set()
    for file in SRC_DIR.rglob("*.rs"):
        text = file.read_text(encoding="utf-8")
        for feat in FEATURE_RE.findall(text):
            features.add(feat)
    return sorted(features)

def update_cargo_toml(features: list[str]) -> None:
    """Rewrite the `[features]` table with the supplied STM32 entries."""
    data = tomllib.loads(TOML_PATH.read_text(encoding="utf-8"))
    feats = data.setdefault("features", {})
    # Remove existing stm32-* keys
    for key in list(feats.keys()):
        if key.startswith("stm32-"):
            feats.pop(key)
    for feat in features:
        feats[feat] = []
    TOML_PATH.write_text(tomli_w.dumps(data), encoding="utf-8")

def main() -> None:
    update_cargo_toml(collect_features())

if __name__ == "__main__":
    main()
