"""Tests for derived suspicion over the rlvgl corpus.

Conformance target: SBC-00-CONCEPTS SBC-INV-19; SBC-00-ADDENDUM-C §8.

The load-bearing test here is that suspicion can FIRE. A pass reporting zero
open suspicions across a 4,700-object corpus is indistinguishable from a pass
whose propagation logic never runs, and the corpus currently supplies almost
no ChangeKind values — so the real corpus cannot exercise this.

Run: python3 scripts/specidx/test_suspect.py
"""

from __future__ import annotations

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))

import scan  # noqa: E402
import suspect  # noqa: E402

DEF_DOC = "family/CONCEPTS.md"
CITING_DOC = "family/PHASE-02.md"

# Amendment lands 2026-06-01; the citing document was last touched 2026-01-01.
AMEND_DATE = "2026-06-01"
OLD_TS = 1735689600  # 2025-01-01
NEW_TS = 1790000000  # 2026-09; genuinely after AMEND_DATE (1780272000)


def _corpus(change_kind: str, touches=("INV-FOO-1",)):
    inv = scan.SpecObject("INV-FOO-1", "invariant", "fam", DEF_DOC, 10, "Original.", {})
    amendment = scan.SpecObject(
        f"{DEF_DOC}#0.2.0", "amendment", "fam", DEF_DOC, 90, "Changed it.",
        {"rev": "0.2.0", "date": AMEND_DATE, "change_kind": change_kind,
         "touches": list(touches), "shape": "block"},
    )
    citations = [
        scan.Citation("INV-FOO-1", DEF_DOC, 10, "fam", is_definition=True),
        scan.Citation("INV-FOO-1", CITING_DOC, 42, "fam", is_definition=False),
    ]
    return [inv, amendment], citations


def test_semantic_amendment_creates_a_suspect():
    """The negative control's control: suspicion must be able to fire."""
    objs, cits = _corpus("semantic")
    r = suspect.derive(objs, cits, {CITING_DOC: OLD_TS, DEF_DOC: OLD_TS}, [])
    assert r["metrics"]["open"] == 1, r["metrics"]
    s = r["suspects"][0]
    assert s["object"] == "INV-FOO-1", s
    assert s["suspect_doc"] == CITING_DOC, s
    assert s["change_kind"] == "semantic", s


def test_editorial_amendment_creates_none():
    """ADDENDUM-C §8.2: editorial does not propagate."""
    objs, cits = _corpus("editorial")
    r = suspect.derive(objs, cits, {CITING_DOC: OLD_TS, DEF_DOC: OLD_TS}, [])
    assert r["metrics"]["open"] == 0, r["metrics"]
    assert r["coverage"]["non_propagating"] == 1, r["coverage"]


def test_dependent_changed_after_amendment_is_acknowledged():
    """A dependent edited after the amendment has already seen it."""
    objs, cits = _corpus("semantic")
    r = suspect.derive(objs, cits, {CITING_DOC: NEW_TS, DEF_DOC: OLD_TS}, [])
    assert r["metrics"]["open"] == 0, r["metrics"]


def test_clearing_trailer_suppresses_suspicion():
    """ADDENDUM-C §8.3: clearings live in commit trailers, not a store."""
    objs, cits = _corpus("semantic")
    cleared = [{"sha": "abc1234", "ts": OLD_TS, "id": "INV-FOO-1",
                "rev": "0.2.0", "target": CITING_DOC}]
    r = suspect.derive(objs, cits, {CITING_DOC: OLD_TS, DEF_DOC: OLD_TS}, cleared)
    assert r["metrics"]["open"] == 0, r["metrics"]
    assert r["clearings_found"] == 1
    assert "VACUOUS" not in r["metrics"], "clearings exist; metrics are not vacuous"


def test_defining_document_is_not_suspect_of_itself():
    """Uses OLD_TS so a suspect genuinely exists — otherwise this passes vacuously."""
    objs, cits = _corpus("semantic")
    r = suspect.derive(objs, cits, {CITING_DOC: OLD_TS, DEF_DOC: OLD_TS}, [])
    assert r["metrics"]["open"] == 1, "fixture must produce a suspect to test exclusion"
    assert all(s["suspect_doc"] != DEF_DOC for s in r["suspects"]), r["suspects"]


def test_untyped_amendment_is_bucketed_not_assumed():
    """Neither flood nor silence: an untyped amendment is reported as such."""
    objs, cits = _corpus("semantic")
    objs[1].attrs["change_kind"] = None
    r = suspect.derive(objs, cits, {CITING_DOC: OLD_TS, DEF_DOC: OLD_TS}, [])
    assert r["metrics"]["open"] == 0, r["metrics"]
    assert r["coverage"]["untypeable"] == 1, r["coverage"]
    assert "WARNING" in r["coverage"], "untyped-dominated corpus must warn"


def test_zero_open_on_real_corpus_is_reported_as_uncomputable():
    """The real corpus reports 0 open — that must not read as clean."""
    root = pathlib.Path(__file__).resolve().parents[2]
    if not (root / ".git").exists():
        return
    r = suspect.compute(root)
    if r["coverage"]["typed_pct"] < 5:
        assert "WARNING" in r["coverage"], "low ChangeKind coverage must warn"
    if not r["clearings_found"]:
        assert "VACUOUS" in r["metrics"], "no clearings must mark metrics vacuous"


def main() -> int:
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    failed = 0
    for t in tests:
        try:
            t()
        except AssertionError as exc:
            failed += 1
            print(f"FAIL {t.__name__}: {exc}")
        except Exception as exc:  # noqa: BLE001
            failed += 1
            print(f"ERROR {t.__name__}: {type(exc).__name__}: {exc}")
        else:
            print(f"ok   {t.__name__}")
    print(f"\n{len(tests) - failed}/{len(tests)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
