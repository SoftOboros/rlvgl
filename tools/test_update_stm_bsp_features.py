"""Temporary-fixture regressions for compatibility-preserving feature sync."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "update_stm_bsp_features", Path(__file__).with_name("update_stm_bsp_features.py")
)
assert SPEC is not None and SPEC.loader is not None
sync = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sync)

MANIFEST = b'''# Preserve compatibility families even when no current board uses them.
[package]
name = "feature-sync-fixture"
version = "0.1.0"

[features]
default = ["native"] # This comment must survive an already-covered sync.
native = []
c_hal = ["dep:cc"]
c_hal_cm4 = ["dep:cc", "cm4"]
cm4 = []
stm32-h7 = ["dep:stm32h7"]
stm32-f1 = []
stm32-f2 = []
stm32-mp = []
stm32-u3 = []
stm32-wb = []
stm32-wba = []
stm32-wl = []

[build-dependencies]
cc = { version = "1", optional = true }

[dependencies.stm32h7]
version = "0.15.1"
optional = true
default-features = false
features = ["stm32h747cm7"]
'''


class FeatureSyncTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = self.root / "Cargo.toml"
        self.manifest.write_bytes(MANIFEST)
        self.toml_patch = patch.object(sync, "TOML_PATH", self.manifest)
        self.toml_patch.start()
        self.addCleanup(self.toml_patch.stop)

    def test_discovered_subset_keeps_compatibility_keys_and_exact_bytes(self) -> None:
        with patch.object(sync.tomli_w, "dumps") as serialize, patch.object(
            Path, "write_text"
        ) as write:
            sync.update_cargo_toml(["stm32-h7", "native", "stm32-h7"])
        serialize.assert_not_called()
        write.assert_not_called()
        self.assertEqual(self.manifest.read_bytes(), MANIFEST)
        features = tomllib.loads(self.manifest.read_text())["features"]
        for family in ("f1", "f2", "mp", "u3", "wb", "wba", "wl"):
            self.assertEqual(features["stm32-" + family], [])
        self.assertEqual(features["c_hal"], ["dep:cc"])
        self.assertEqual(features["c_hal_cm4"], ["dep:cc", "cm4"])
        self.assertEqual(features["stm32-h7"], ["dep:stm32h7"])

    def test_missing_family_addition_preserves_every_existing_field(self) -> None:
        expected = tomllib.loads(MANIFEST.decode())
        expected["features"]["stm32-h5"] = []
        sync.update_cargo_toml(["stm32-h7", "stm32-h5", "stm32-h5", "ignored"])
        first = self.manifest.read_bytes()
        self.assertEqual(tomllib.loads(first.decode()), expected)
        with patch.object(sync.tomli_w, "dumps") as serialize, patch.object(
            Path, "write_text"
        ) as write:
            sync.update_cargo_toml(["stm32-h5", "stm32-h7"])
        serialize.assert_not_called()
        write.assert_not_called()
        self.assertEqual(self.manifest.read_bytes(), first)

    def test_no_discovered_families_does_not_rewrite_manifest(self) -> None:
        with patch.object(sync.tomli_w, "dumps") as serialize, patch.object(
            Path, "write_text"
        ) as write:
            sync.update_cargo_toml([])
        serialize.assert_not_called()
        write.assert_not_called()
        self.assertEqual(self.manifest.read_bytes(), MANIFEST)

    def test_discovery_is_recursive_sorted_and_stm32_only(self) -> None:
        sources = self.root / "src"
        (sources / "nested").mkdir(parents=True)
        (sources / "lib.rs").write_text(
            '#[cfg(feature = "stm32-h7")]\n'
            '#[cfg(feature = "c_hal")]\n'
            '#[cfg(feature = "stm32-h7")]\n'
        )
        (sources / "nested" / "board.rs").write_text(
            '#[cfg(any(feature="stm32-f4", feature = "native"))]\n'
        )
        (sources / "ignored.txt").write_text('feature = "stm32-u5"\n')
        with patch.object(sync, "SRC_DIR", sources):
            self.assertEqual(sync.collect_features(), ["stm32-f4", "stm32-h7"])


if __name__ == "__main__":
    unittest.main()
