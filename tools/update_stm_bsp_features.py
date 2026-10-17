#!/usr/bin/env python3
"""Synchronize ``stm32-*`` feature options for the STM BSP crate.

Scans the generated BSP sources for ``#[cfg(feature = "stm32-*")]``
entries and adds missing feature keys to ``Cargo.toml``. Existing declarations
and dependency vectors are retained for compatibility. If all discovered
features are already declared, the manifest bytes are left untouched.
"""
from __future__ import annotations

import re
from pathlib import Path
import tomllib
import tomli_w

ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = ROOT / "chips" / "stm" / "bsps" / "src"
TOML_PATH = SRC_DIR.parent / "Cargo.toml"

FEATURE_RE = re.compile(r'feature\s*=\s*"([^"]+)"')

def collect_features() -> list[str]:
    """Return sorted by feature names present in the BSP sources."""
    features: set[str] = set()
    for file in SRC_DIR.rglob("*.rs"):
        text = file.read_text(encoding="utf-8")
        for feat in FEATURE_RE.findall(text):
            if feat.startswith("stm32-"):
                features.add(feat)
    return sorted(features)

def update_cargo_toml(features: list[str]) -> None:
    """Add missing STM32 feature keys without removing existing declarations."""
    data = tomllib.loads(TOML_PATH.read_text(encoding="utf-8"))
    feats = data.setdefault("features", {})
    missing = sorted(
        {feat for feat in features if feat.startswith("stm32-") and feat not in feats}
    )
    if not missing:
        return
    for feat in missing:
        feats[feat] = []
    TOML_PATH.write_text(tomli_w.dumps(data), encoding="utf-8")

def main() -> None:
    update_cargo_toml(collect_features())

if __name__ == "__main__":
    main()
