"""Tests for the MPY introspection coverage ledger.

MPY-01 §11.1 non-goal 1 scopes this phase to "a baseline and tests for its
schema".  These tests enforce the ledger's schema plus the MPY-01 invariants
that a JSON Schema alone cannot express:

* ``INV-MPY-01-1`` — every row is pinned to the inherited LPAR-01 §2 baseline.
* ``INV-MPY-01-2`` — one versioned row per claim, ids stable and unique.
* ``INV-MPY-01-3`` — missing and unsupported stay distinct.
* ``INV-MPY-01-6`` — the ledger stays a claim ledger; §6 of the Markdown and
  the JSON cannot drift apart.
"""

import json
import re
from pathlib import Path

import jsonschema

SCHEMA_PATH = Path("schemas/mpy-coverage.schema.json")
LEDGER_PATH = Path("docs/concepts/MPY-COVERAGE-MATRIX.json")
MPY01_PATH = Path("docs/concepts/MPY-01-INTROSPECTION-BASELINE.md")
LPAR01_PATH = Path("docs/concepts/LPAR-01-BASELINE.md")


def _ledger():
    return json.loads(LEDGER_PATH.read_text())


def _markdown_rows():
    """Return {row_id: (level, surface, owners)} parsed from MPY-01 §6."""
    text = MPY01_PATH.read_text()
    section = text.split("## 6.")[1].split("## 7.")[0]
    rows = {}
    for line in section.splitlines():
        if not line.startswith("| MPY-BL-"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        row_id, level, surface, _reference, _status, owner = cells
        owners = tuple(
            f"MPY-{part}" if not part.startswith("MPY-") else part
            for part in owner.replace("MPY-", "").split("/")
        )
        rows[row_id] = (level, surface, owners)
    return rows


def test_ledger_matches_schema():
    schema = json.loads(SCHEMA_PATH.read_text())
    jsonschema.validate(_ledger(), schema)


def test_baseline_pin_matches_lpar01():
    """INV-MPY-01-1: MPY inherits the LPAR-01 §2 pin and never advances it."""
    baseline = _ledger()["lvgl_baseline"]
    lpar = LPAR01_PATH.read_text()
    assert baseline["source_commit"] in lpar
    assert baseline["effective_target_label"] in lpar
    # The same pin is restated in MPY-01 §0; the two must agree.
    assert baseline["source_commit"] in MPY01_PATH.read_text()


def test_row_ids_are_unique_and_dense():
    """INV-MPY-01-2: ids are stable claim units, never re-bound or reused."""
    ids = [row["id"] for row in _ledger()["rows"]]
    assert len(ids) == len(set(ids))
    numbers = sorted(int(re.fullmatch(r"MPY-BL-(\d{3})", i).group(1)) for i in ids)
    assert numbers == list(range(1, len(numbers) + 1))


def test_ledger_agrees_with_markdown_section_6():
    """INV-MPY-01-6: the ledger and the ratified §6 table cannot drift."""
    md = _markdown_rows()
    ledger = {row["id"]: row for row in _ledger()["rows"]}
    assert set(md) == set(ledger)
    for row_id, (level, surface, owners) in md.items():
        assert ledger[row_id]["level"] == level, row_id
        assert ledger[row_id]["surface"] == surface, row_id
        assert tuple(ledger[row_id]["owners"]) == owners, row_id


def test_baseline_carries_no_evidence_backed_claims():
    """MPY-01 §6: MPY-01 freezes row scope and status.

    Later phases add per-profile claims *with cited evidence*; the ratified
    baseline itself claims nothing, so no row may ship a `current` status
    without having gone through a later phase.
    """
    for row in _ledger()["rows"]:
        assert row["baseline_status"] != "current", row["id"]


def test_current_claims_cite_evidence():
    """MPY-01 §8: a row promoted to `current` cites deterministic evidence.

    Vacuously true at baseline (no claims exist yet).  It becomes load-bearing
    the moment MPY-03 onward starts promoting rows, which is the point.
    """
    for row in _ledger()["rows"]:
        for claim in row["claims"]:
            if claim["status"] == "current":
                assert claim["evidence"], (row["id"], claim["profile"])


def test_unsupported_claims_name_the_rejecting_capability():
    """INV-MPY-01-3: unsupported is not a synonym for missing."""
    for row in _ledger()["rows"]:
        for claim in row["claims"]:
            if claim["status"] == "unsupported":
                assert claim.get("unsupported_reason"), (row["id"], claim["profile"])


def test_no_row_claims_a_profile_twice():
    for row in _ledger()["rows"]:
        profiles = [claim["profile"] for claim in row["claims"]]
        assert len(profiles) == len(set(profiles)), row["id"]
