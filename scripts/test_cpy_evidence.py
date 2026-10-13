#!/usr/bin/env python3
"""Focused tests for the CPY baseline and dependency firewall."""

from __future__ import annotations

import copy
import importlib.util
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).with_name("cpy_evidence.py")
SPEC = importlib.util.spec_from_file_location("cpy_evidence", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
cpy_evidence = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cpy_evidence)


class BaselineTests(unittest.TestCase):
    """Exercise real baseline evidence and deliberate failure controls."""

    def test_baseline_is_valid(self) -> None:
        """The committed baseline and its evidence artifacts agree."""

        manifest = cpy_evidence.validate_baseline()
        self.assertEqual(manifest["schema_version"], "CPY-BASELINE-1")

    def test_legacy_authority_cannot_invent_a_revision(self) -> None:
        """Legacy LPAR authority stays source-commit keyed, not fake-semver keyed."""

        manifest = cpy_evidence._load_json(cpy_evidence.DEFAULT_MANIFEST)
        legacy = next(
            authority
            for authority in manifest["authorities"]
            if authority["revision_basis"] == "source-commit"
        )
        legacy["revision"] = "0.0.0"
        with self.assertRaises(cpy_evidence.EvidenceError):
            cpy_evidence._validate_schema(manifest)

    def test_coordination_closeout_is_valid(self) -> None:
        """The first shared migration wave has a coherent closeout record."""

        manifest = cpy_evidence._load_json(cpy_evidence.DEFAULT_MANIFEST)
        closeout = cpy_evidence.validate_closeout(manifest)
        self.assertEqual(closeout["state"], "closed")

    def test_unaccepted_mpy_bridge_cannot_be_relabelled(self) -> None:
        """The closeout cannot silently accept the next MPY protocol slice."""

        manifest = cpy_evidence._load_json(cpy_evidence.DEFAULT_MANIFEST)
        closeout = cpy_evidence._load_json(cpy_evidence.CLOSEOUT_PATH)
        closeout["mpy_disposition"]["next_pcdn"]["state"] = "accepted"
        with self.assertRaises(cpy_evidence.EvidenceError):
            cpy_evidence._verify_closeout(manifest, closeout)


class FirewallTests(unittest.TestCase):
    """Exercise the live graph and a synthetic prohibited edge."""

    @classmethod
    def setUpClass(cls) -> None:
        """Load Cargo metadata once for the focused firewall tests."""

        cls.metadata = cpy_evidence.load_cargo_metadata(locked=False)

    def test_live_workspace_passes(self) -> None:
        """The current workspace respects CPY-02 dependency direction."""

        self.assertEqual(cpy_evidence.dependency_firewall_violations(self.metadata), [])

    def test_pyo3_edge_into_api_is_rejected(self) -> None:
        """The negative control proves a PyO3 edge into rlvgl-api is detected."""

        metadata = copy.deepcopy(self.metadata)
        api = next(package for package in metadata["packages"] if package["name"] == "rlvgl-api")
        fake_id = "registry+https://github.com/rust-lang/crates.io-index#pyo3@999.0.0"
        metadata["packages"].append(
            {
                "id": fake_id,
                "name": "pyo3",
                "version": "999.0.0",
                "features": {},
                "dependencies": [],
            }
        )
        metadata["resolve"]["nodes"].append({"id": fake_id, "deps": []})
        api_node = next(node for node in metadata["resolve"]["nodes"] if node["id"] == api["id"])
        api_node["deps"].append(
            {
                "name": "pyo3",
                "pkg": fake_id,
                "dep_kinds": [{"kind": None, "target": None}],
            }
        )
        violations = cpy_evidence.dependency_firewall_violations(metadata)
        self.assertIn("rlvgl-api transitively depends on forbidden package pyo3", violations)


if __name__ == "__main__":
    unittest.main(verbosity=2)
