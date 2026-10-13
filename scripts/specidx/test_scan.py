"""Tests for the rlvgl spec-corpus scanner.

Conformance target: SBC-00-ADDENDUM-D §7.2 D-C1 — a parser MUST accept all
three authored change-log shapes, and D-C2 — fields absent from an authored
form are marked inferred or omitted, never fabricated.

Run: python3 scripts/specidx/test_scan.py   (or pytest)
"""

from __future__ import annotations

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))

import scan  # noqa: E402

BLOCK_FORM = """
## §9 Change Log

### 0.6.0 — 2026-08-04 — ratified

**Author:** I. Abbott
**Change kind:** semantic
**Touches:** INV-FOO-4, INV-FOO-5, §7.2
**Commits:** a1b2c3d, e4f5a6b
**Summary:** Scoped INV-FOO-4 to extracted dimensional values.

#### Rationale

Five phases had each invented a vocabulary for one concept. Considered and
rejected: per-phase adapters, which preserve the collisions.

What did not change is the finding worth recording.

### 0.5.0 — 2026-08-01 — amended

**Author:** I. Abbott
**Change kind:** editorial
**Touches:** none
**Summary:** Fixed two broken links in §13.
"""

COMPACT_FORM = """
## §15 Change Log

| Rev | Date | Author | Status | Change kind | Touches | Summary |
|---|---|---|---|---|---|---|
| 0.5.1 | 2026-08-01 | I. Abbott | amended | editorial | none | Fixed links. |
"""

LEGACY_BULLET = """
## §15 Change Log

- **2026-06-16** - **Ratified.** User confirmed the foundation.
- **2026-06-17** - **Amended.** Added a reconciliation row.
"""


def _entries(text):
    return scan.parse_changelog_entries(text.splitlines(), "test.md", "testfam")


def test_block_form_extracts_all_declared_fields():
    """D-C1: the block form ADDENDUM-D freezes must parse."""
    ents = [e for e in _entries(BLOCK_FORM) if e.kind == "amendment"]
    assert len(ents) == 2, f"expected 2 amendments, got {len(ents)}"

    top = ents[0]
    a = top.attrs
    assert a["rev"] == "0.6.0", a["rev"]
    assert a["date"] == "2026-08-04", a["date"]
    assert a["status"] == "ratified", a["status"]
    assert a["author"] == "I. Abbott", a["author"]
    assert a["change_kind"] == "semantic", a["change_kind"]
    assert a["touches"] == ["INV-FOO-4", "INV-FOO-5", "§7.2"], a["touches"]
    assert a["commits"] == ["a1b2c3d", "e4f5a6b"], a["commits"]
    assert a["shape"] == "block", a["shape"]
    # Declared fields must not be marked inferred (D-C2).
    assert a["field_provenance"] == "declared", a["field_provenance"]


def test_block_form_emits_linked_rationale_object():
    """SBC-INV-17: semantic amendments carry a rationale, linked by motivates."""
    objs = _entries(BLOCK_FORM)
    rats = [o for o in objs if o.kind == "rationale"]
    assert len(rats) == 1, f"expected 1 rationale, got {len(rats)}"
    r = rats[0]
    assert "rejected" in r.text.lower()
    assert r.attrs["motivates"] == ["INV-FOO-4", "INV-FOO-5", "§7.2"], r.attrs["motivates"]
    assert r.obj_id.endswith("#0.6.0-rationale"), r.obj_id


def test_editorial_entry_needs_no_rationale():
    """ChangeKind editorial does not trigger the SBC-INV-17 obligation."""
    ents = [e for e in _entries(BLOCK_FORM) if e.kind == "amendment"]
    editorial = [e for e in ents if e.attrs["change_kind"] == "editorial"][0]
    assert editorial.attrs["touches"] == [], editorial.attrs["touches"]
    assert editorial.attrs["has_rationale"] is False


def test_compact_form_parses():
    """D-C1: the §2.2 compact table row."""
    ents = [e for e in _entries(COMPACT_FORM) if e.kind == "amendment"]
    assert len(ents) == 1, f"expected 1, got {len(ents)}"
    a = ents[0].attrs
    assert a["shape"] == "compact", a["shape"]
    assert a["change_kind"] == "editorial", a["change_kind"]
    assert a["touches"] == [], a["touches"]


def test_legacy_bullet_marks_absent_fields_inferred_not_fabricated():
    """D-C2: bullets carry no rev or author; those must be absent, not invented."""
    ents = [e for e in _entries(LEGACY_BULLET) if e.kind == "amendment"]
    assert len(ents) == 2, f"expected 2, got {len(ents)}"
    a = ents[0].attrs
    assert a["shape"] == "legacy-bullet", a["shape"]
    assert a["rev"] is None, f"rev must be absent, got {a['rev']!r}"
    assert a["author"] is None, f"author must be absent, got {a['author']!r}"
    assert a["change_kind"] is None, "change_kind must not be fabricated"
    assert a["field_provenance"] == "inferred", a["field_provenance"]


def test_addendum_d_parses_as_block_form():
    """The document that froze the shape is its own conformance test."""
    root = pathlib.Path(__file__).resolve().parents[2]
    doc = root / "docs/spec-before-code/docs/SBC-00-ADDENDUM-D.md"
    if not doc.exists():
        return  # doc not present in this checkout; unit tests above still bind
    ents = [
        e
        for e in scan.parse_changelog_entries(
            doc.read_text(encoding="utf-8").splitlines(), str(doc), "sbc"
        )
        if e.kind == "amendment"
    ]
    assert ents, "ADDENDUM-D's own change log did not parse"
    assert all(e.attrs["shape"] == "block" for e in ents), [e.attrs["shape"] for e in ents]
    assert any(e.attrs["rev"] == "0.2.0" for e in ents), [e.attrs["rev"] for e in ents]


LEGACY_TABLE_WITH_PIPES = """
## §15 Change Log

| Rev | Date | Author | Status | Summary |
|---|---|---|---|---|
| 0.1.13 | 2026-05-30 | I. Abbott | Ratified | Registers `a` (`error`, default `allow`) | promotes fallback | adds policy |
"""

FENCED_EXAMPLE = """
## §15 Change Log

```markdown
### 9.9.9 — 2099-01-01 — ratified

**Change kind:** semantic
**Touches:** INV-FOO-1
```

### 0.1.0 — 2026-08-01 — drafted

**Author:** I. Abbott
**Change kind:** editorial
**Touches:** none
**Summary:** Real entry.
"""


def test_legacy_row_with_pipes_is_not_read_as_compact():
    """Regression: literal | in a summary inflated the cell count.

    TODO-SSP-04-PUBLICATION-RENDERING.md:607 parsed as compact form and
    carried a fragment of prose as its change_kind, which then entered the
    index and the suspicion pass as a typed amendment.
    """
    ents = [e for e in _entries(LEGACY_TABLE_WITH_PIPES) if e.kind == "amendment"]
    assert len(ents) == 1, f"expected 1, got {len(ents)}"
    a = ents[0].attrs
    assert a["shape"] == "legacy-table", a["shape"]
    assert a["change_kind"] is None, f"change_kind must be absent, got {a['change_kind']!r}"


def test_fenced_examples_are_not_corpus_content():
    """A document teaching the shape contains example entries in fences."""
    ents = [e for e in _entries(FENCED_EXAMPLE) if e.kind == "amendment"]
    assert len(ents) == 1, f"fenced example was counted; got {len(ents)}"
    assert ents[0].attrs["rev"] == "0.1.0", ents[0].attrs["rev"]


# Verbatim from TODO-MCAD-00-CONCEPTS.md rev 0.3.0.  The author states that an
# invariant was explicitly NOT amended; an extractor that records the opposite
# writes false history into the one system whose value is trustworthy history.
MCAD_DISCLAIMER = (
    "\u00a711 adds INV-MCAD-9/10 and records why **INV-MCAD-5 needed no "
    "amendment** for T3: the invariant governs authority."
)
MCAD_REAL_ACTION = (
    "**(1) The frame model (\u00a77.2, new INV-MCAD-9).** Six coordinate frames "
    "had been introduced across four phase docs."
)


def test_touches_proposal_respects_not_amended_disclaimers():
    """Regression: 'needed no amendment' was proposed as a touch."""
    am = scan.SpecObject(
        "d#0.3.0", "amendment", "mcad", "d.md", 1, MCAD_DISCLAIMER,
        {"rev": "0.3.0", "date": "2026-07-25"},
    )
    props = scan.propose_touches([am])
    touched = props[0]["proposed_touches"] if props else []
    assert "INV-MCAD-5" not in touched, f"disclaimer read as an action: {touched}"


def test_touches_proposal_still_finds_real_actions():
    """The guard above must not silence genuine additions."""
    am = scan.SpecObject(
        "d#0.3.0", "amendment", "mcad", "d.md", 1, MCAD_REAL_ACTION,
        {"rev": "0.3.0", "date": "2026-07-25"},
    )
    props = scan.propose_touches([am])
    assert props, "real action produced no proposal"
    assert "INV-MCAD-9" in props[0]["proposed_touches"], props[0]["proposed_touches"]


def test_touches_proposal_never_overwrites_declared_values():
    """An amendment that already declares Touches: is left alone."""
    am = scan.SpecObject(
        "d#0.3.0", "amendment", "mcad", "d.md", 1, MCAD_REAL_ACTION,
        {"rev": "0.3.0", "date": "2026-07-25", "touches": ["INV-MCAD-1"]},
    )
    assert scan.propose_touches([am]) == []


def _tiny_corpus():
    """A minimal in-memory scan result for index tests."""
    objs = [
        scan.SpecObject("INV-FOO-2", "invariant", "fam", "b.md", 5, "Second.", {}),
        scan.SpecObject("INV-FOO-1", "invariant", "fam", "a.md", 9, "First.", {}),
        scan.SpecObject("fam:term:x", "term", "fam", "a.md", 3, "A term.", {}),
    ]
    return {"docs": [], "objects": objs, "citations": []}


def test_index_is_deterministic():
    """SBC-INV-20 rests on this: same corpus -> byte-identical output."""
    a = scan._render(scan.build_index(_tiny_corpus())["fam"])
    b = scan._render(scan.build_index(_tiny_corpus())["fam"])
    assert a == b, "index rendering is not deterministic"
    # and stable regardless of input ordering
    c = _tiny_corpus()
    c["objects"].reverse()
    assert scan._render(scan.build_index(c)["fam"]) == a, "index depends on input order"


def test_index_carries_no_generation_metadata():
    """The index MUST NOT embed its own commit or a timestamp.

    Committing the index changes HEAD, so an embedded HEAD SHA would make the
    no-op check unsatisfiable by construction.
    """
    payload = scan._render(scan.build_index(_tiny_corpus()))
    banned = ("commit", "generated_at", "timestamp", "head_sha", "generated")
    low = payload.lower()
    for word in banned:
        assert word not in low, f"index embeds generation metadata: {word!r}"


def test_check_index_detects_drift(tmpdir=None):
    """The no-op check must actually be able to fail."""
    import shutil
    import tempfile

    d = pathlib.Path(tempfile.mkdtemp())
    try:
        idx = scan.build_index(_tiny_corpus())
        scan.emit_index(idx, d)
        assert scan.check_index(idx, d) == [], "clean index reported drift"

        grown = _tiny_corpus()
        grown["objects"].append(
            scan.SpecObject("INV-FOO-3", "invariant", "fam", "c.md", 1, "Third.", {})
        )
        drift = scan.check_index(scan.build_index(grown), d)
        assert drift, "drift not detected after adding an object"
        assert any("fam.json" in x for x in drift), drift
    finally:
        shutil.rmtree(d, ignore_errors=True)


def test_check_index_reports_missing_directory():
    missing = pathlib.Path("/nonexistent-specidx-dir-for-test")
    drift = scan.check_index(scan.build_index(_tiny_corpus()), missing)
    assert drift and "does not exist" in drift[0], drift


def test_document_paths_are_posix_on_every_platform():
    """SBC-INV-20 rests on this too: the index stores a source path per object.

    A native-flavour ``Path`` stringifies with backslashes on Windows, so
    regenerating the index there rewrites every source path in the corpus and
    turns the no-op check into a whole-corpus diff — and ``--emit-index`` would
    commit that. Uses a reserved illustration prefix per PREFIX-REGISTRY.
    """
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        root = pathlib.Path(td)
        doc_path = root / "docs" / "todo" / "fam" / "TODO-FOO-00-CONCEPTS.md"
        doc_path.parent.mkdir(parents=True)
        doc_path.write_text(
            "# T\n\n## §9 Frozen Invariants\n\n"
            "| Id | Invariant | Verified by |\n|---|---|---|\n"
            "| **INV-FOO-1** | A thing MUST hold. | review |\n",
            encoding="utf-8",
        )
        doc, objects, _citations = scan.parse_document(doc_path, root)

    assert objects, "fixture produced no objects — test would pass vacuously"
    assert doc.path == "docs/todo/fam/TODO-FOO-00-CONCEPTS.md", (
        f"document path is not posix: {doc.path!r}"
    )
    for o in objects:
        assert "\\" not in o.doc, f"object path is not posix: {o.doc!r}"


def test_legacy_row_naming_its_change_kind_in_prose_is_not_a_header():
    """A data row is not a column header just because it says "change kind".

    The compact-form detector keys on the phrase appearing in a table line. A
    legacy-table summary that writes "**Change kind:** semantic" — the very
    convention SBC-00-CONCEPTS adopted for its 0.8.0 and 0.9.0 ratifications —
    was therefore read as a header and skipped, dropping the amendment from the
    corpus entirely. A header never opens with a revision.
    """
    doc = """
## §15 Change Log

| Rev | Date | Author | Status | Summary |
|---|---|---|---|---|
| 0.1.0 | 2026-01-01 | A | DRAFT | Initial draft. |
| 0.2.0 | 2026-01-02 | A | Ratified | **Change kind:** semantic — adopts a rule. |
"""
    entries = [
        o
        for o in scan.parse_changelog_entries(doc.splitlines(), "d.md", "fam")
        if o.kind == "amendment"
    ]
    revs = [o.attrs.get("rev") for o in entries]
    assert revs == ["0.1.0", "0.2.0"], f"amendment dropped by header misdetection: {revs}"



RLVGL_INVARIANT_IDS = (
    "INV-C1",
    "INV-D16",
    "INV-DPR-10",
    "INV-SL5",
    "INV-MC8",
    "INV-BEETLE-00-8",
    "INV-BEETLE-IDF-5-4",
    "INV-SCTD02-2",
)


def test_rlvgl_authored_invariant_ids_parse():
    """The local index preserves every ratified rlvgl identifier shape."""
    for obj_id in RLVGL_INVARIANT_IDS:
        assert scan.RE_INVARIANT.fullmatch(obj_id), f"unrecognized rlvgl id: {obj_id}"


def test_rlvgl_phase_documents_get_stable_families():
    cases = {
        "docs/concepts/DCB-00-CONCEPTS.md": "dcb",
        "docs/concepts/DCB-01b-A.md": "dcb",
        "docs/beetle-esp32p4/BEETLE-08-DEMO-INTEGRATION.md": "beetle",
        "docs/beetle-esp32p4/ERRATA.md": "beetle",
        "docs/beetle-esp32p4-idf/BEETLE-IDF-00-CONCEPTS.md": "beetle-idf",
        "docs/crates-ci/CRATES-CI-00-CONCEPTS.md": "crates-ci",
        "chipdb/rlvgl-chips-microchip/docs/CHIPS-MICROCHIP-05-LINKER.md":
            "chips-microchip",
    }
    for path, expected in cases.items():
        got = scan.family_of(pathlib.PurePosixPath(path))
        assert got == expected, f"{path}: expected {expected}, got {got}"


def test_rlvgl_committed_index_is_non_vacuous_and_current():
    """A passing local suite must scan rlvgl and prove committed-index parity."""
    root = pathlib.Path(__file__).resolve().parents[2]
    data = scan.scan(root)
    assert data["docs"], "rlvgl corpus scan was vacuous"
    assert data["objects"], "rlvgl corpus produced no index objects"
    index = scan.build_index(data)
    assert index["_manifest"]["total_objects"] > 0
    drift = scan.check_index(index, root / scan.DEFAULT_INDEX_DIR)
    assert drift == [], "committed rlvgl index drift: " + "; ".join(drift)

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
