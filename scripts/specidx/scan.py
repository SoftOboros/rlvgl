#!/usr/bin/env python3
"""Read-only scanner over the rlvgl spec-before-code corpus.

Extracts spec objects (invariants, errata, open questions, glossary terms,
acceptance gates, non-goals) and the citation graph between them, then reports
structural findings: format drift, unverified invariants, orphan citations,
and cross-family term collisions.

The report path is diagnostic: the existing corpus is a baseline and is not
declared conformant merely because it can be indexed. The emit/check path is
the local source for the committed dashboard input.

Usage:
    python scripts/specidx/scan.py [--root DOCS_ROOT] [--json OUT.json]
"""

from __future__ import annotations

import argparse
import collections
import dataclasses
import json
import pathlib
import re
import sys

# --------------------------------------------------------------------------
# Corpus scope
# --------------------------------------------------------------------------

# The parent softoboros index intentionally excludes submodules. rlvgl owns
# this corpus and scans it directly so a standalone checkout has the same gate.
SCAN_DIRS = [
    "docs",
    "chipdb",
]

# Generated, vendored, archived, and nested-upstream trees are not authority.
EXCLUDE_PARTS = {
    "frontend",
    "node_modules",
    ".git",
    "archived",
    "target",
    "_generated",
    "vendor",
    "lvgl",
}

# --------------------------------------------------------------------------
# Object grammars (SBC-00-CONCEPTS §8)
# --------------------------------------------------------------------------

# rlvgl predates the parent's canonical INV-<PREFIX>-<N> spelling and has
# ratified identifiers such as INV-D1, INV-DPR-10, INV-BEETLE-00-1, and
# INV-BEETLE-IDF-5-4. The local index preserves those authored handles.
_INV_ID = r"(?:INV-[A-Z][A-Z0-9-]*\d|[A-Z][A-Z0-9-]*-INV-?\d+)"
RE_INVARIANT = re.compile(rf"\b{_INV_ID}\b")
RE_ERRATA_ID = re.compile(r"\bERRATA-(\d+)\b")
RE_PCDN = re.compile(r"\bPCDN-[A-Z][A-Z0-9-]*-\d+\b")
RE_EOQ = re.compile(r"\bEOQ-(\d+)-ERRATA-(\d+)\b")

# Definition sites (as opposed to citations).  The corpus uses four distinct
# shapes for the same semantic act — "this line defines this invariant".  A
# definition is an id rendered in bold at the head of a table cell, a list
# item, or a heading.  Cataloguing them is itself finding F10.

RE_INV_DEF_ROW = re.compile(rf"^\|\s*\*\*({_INV_ID})\*\*\s*\|(.*)$")
RE_INV_DEF_BULLET = re.compile(
    rf"^\s*(?:[-*]|\d+\.)\s+\*\*({_INV_ID})\s*[—\-–:.]?\s*([^*]*)\*\*\.?\s*(.*)$"
)
RE_INV_DEF_HEADING = re.compile(rf"^#{{2,4}}\s+\*{{0,2}}({_INV_ID})\b\s*[—\-–:.]?\s*(.*)$")
RE_ERRATA_DEF = re.compile(r"^#{2,3}\s+(ERRATA-\d+)\s*[—-]?\s*(.*)$")
RE_GLOSSARY_ROW = re.compile(r"^\|\s*\*\*([^*|]{2,60})\*\*\s*\|\s*([^|]*)\|\s*([^|]*)\|")
RE_ACCEPTANCE_GATE = re.compile(r"^\s*-\s*\[([ xX])\]\s+(.*)$")
RE_NONGOAL = re.compile(r"^\s*(\d+)\.\s+\*\*([^*]+)\*\*\.?\s*(.*)$")

# Document header attributes.
RE_HEADER_ATTR = re.compile(r"^\*\*([A-Za-z][A-Za-z ]{2,30}):\*\*\s*(.*)$")

# Headings, normalised to a section number where one is present.
RE_HEADING = re.compile(r"^(#{1,4})\s+(.*)$")
RE_SECTION_NUM = re.compile(r"^§?\s*(\d+)(?:\.|\s|$)")

# Change-log row shapes.
RE_CL_TABLE_ROW = re.compile(
    r"^\|\s*([0-9]+\.[0-9]+\.[0-9]+|Exec-[0-9.]+)\s*\|\s*(\d{4}-\d{2}-\d{2})\s*\|"
)
RE_CL_BULLET_ROW = re.compile(r"^\s*[-*]\s+\*\*(\d{4}-\d{2}-\d{2})")

# Section titles we care about, matched loosely against heading text.
SECTION_HINTS = {
    "glossary": 3,
    "acceptance": 12,
    "non-goal": 11,
    "change log": 15,
    "invariant": 9,
}

# The six authority-matrix column headers are structural, not vocabulary.
AUTHORITY_AXES = {
    "upstream authority",
    "local representation",
    "mutation rights",
    "divergence policy",
    "downstream consumers",
    "conformance test owner",
}

# --------------------------------------------------------------------------
# Change-log grammars (SBC-00-ADDENDUM-D §2; conformance target D-C1)
# --------------------------------------------------------------------------

RE_CL_SECTION = re.compile(r"^(#{2,3})\s+.*change\s*log", re.IGNORECASE)
RE_CL_BLOCK_HEAD = re.compile(
    r"^#{3,4}\s+(?P<rev>[0-9]+\.[0-9]+\.[0-9]+|Exec-[0-9.]+)\s*[—–-]\s*"
    r"(?P<date>\d{4}-\d{2}-\d{2})\s*[—–-]\s*(?P<status>[A-Za-z][A-Za-z -]*?)\s*$"
)
RE_CL_FIELD = re.compile(
    r"^\*\*(Author|Change kind|Touches|Commits|Summary):\*\*\s*(.*)$", re.IGNORECASE
)
RE_CL_RATIONALE_HEAD = re.compile(r"^#{4,5}\s+Rationale\s*$", re.IGNORECASE)

# Frozen enums — ADDENDUM-D §3 and ADDENDUM-C §8.2.
AMENDMENT_STATUS = {"drafted", "ratified", "amended", "execution", "superseded"}
CHANGE_KIND = {"editorial", "clarification", "semantic", "scope", "retirement"}
# ADDENDUM-C INV-C-3 / SBC-INV-17: these kinds oblige a rationale.
RATIONALE_REQUIRED = {"semantic", "scope", "retirement"}

# Prefixes reserved for illustrations. A document teaching the identifier
# grammar must write example ids; without this set those examples appear as
# orphan citations.
EXAMPLE_PREFIXES = {"FOO", "BAR", "BAZ", "EXAMPLE"}


def is_example_id(obj_id: str) -> bool:
    if not RE_INVARIANT.fullmatch(obj_id):
        return False
    return any(
        obj_id.startswith(f"INV-{prefix}") or obj_id.startswith(f"{prefix}-INV")
        for prefix in EXAMPLE_PREFIXES
    )


RFC2119 = re.compile(r"\b(MUST NOT|MUST|SHALL NOT|SHALL|SHOULD NOT|SHOULD|MAY|RECOMMENDED)\b")


# --------------------------------------------------------------------------
# Data model
# --------------------------------------------------------------------------


@dataclasses.dataclass
class SpecObject:
    obj_id: str
    kind: str  # invariant | errata | pcdn | eoq | term | gate | nongoal
    family: str
    doc: str
    line: int
    text: str = ""
    attrs: dict = dataclasses.field(default_factory=dict)

    def to_dict(self) -> dict:
        return dataclasses.asdict(self)


@dataclasses.dataclass
class Citation:
    obj_id: str
    doc: str
    line: int
    family: str
    is_definition: bool = False


@dataclasses.dataclass
class Document:
    path: str
    family: str
    header: dict = dataclasses.field(default_factory=dict)
    sections: set = dataclasses.field(default_factory=set)
    changelog_shape: str = "none"  # table | bullet | mixed | none
    changelog_entries: int = 0
    is_concepts: bool = False


RE_PHASE_DOC = re.compile(
    r"^(?:TODO-)?(?P<family>[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*)-"
    r"\d{2}[A-Z]?(?:-|\.|$)",
    re.IGNORECASE,
)


def family_of(rel_path: pathlib.Path) -> str:
    """Return an rlvgl initiative family, falling back to its docs lane."""
    phase = RE_PHASE_DOC.match(rel_path.name)
    if phase:
        return phase.group("family").lower()

    parts = rel_path.parts
    if parts and parts[0] == "chipdb":
        for part in parts:
            if part.startswith("rlvgl-chips-"):
                return part.removeprefix("rlvgl-")

    if parts and parts[0] == "docs":
        if len(parts) > 1:
            lane = parts[1].lower()
            return {
                "beetle-esp32p4": "beetle",
                "beetle-esp32p4-idf": "beetle-idf",
            }.get(lane, lane)
        return "docs"

    return "unknown"

# --------------------------------------------------------------------------
# Parsing
# --------------------------------------------------------------------------


def _split_list(value: str):
    """Parse a Touches:/Commits: value.  'none' is a declared empty set."""
    v = value.strip()
    if not v or v.lower() in {"none", "n/a", "—", "-"}:
        return []
    # "none — new family" is a declared empty set with an explanation, not an
    # object called "none — new family".
    if re.match(r"^(none|n/a)\b", v, re.IGNORECASE):
        return []
    return [part.strip() for part in v.split(",") if part.strip()]


def _amendment(doc: str, family: str, line: int, shape: str, **fields) -> SpecObject:
    rev = fields.get("rev")
    ck = fields.get("change_kind")
    status = fields.get("status")
    return SpecObject(
        obj_id=f"{doc}#{rev}" if rev else f"{doc}#{fields.get('date')}@{line}",
        kind="amendment",
        family=family,
        doc=doc,
        line=line,
        text=fields.get("summary") or "",
        attrs={
            "shape": shape,
            # Block and compact forms declare their fields; legacy shapes do
            # not carry them at all, so absent values stay None rather than
            # being invented (ADDENDUM-D §7.2 D-C2).
            "field_provenance": "declared" if shape in {"block", "compact"} else "inferred",
            "conformant_status": status in AMENDMENT_STATUS if status else None,
            "conformant_change_kind": ck in CHANGE_KIND if ck else None,
            "rationale_required": ck in RATIONALE_REQUIRED if ck else None,
            **fields,
        },
    )


def parse_changelog_entries(lines, doc: str, family: str) -> list:
    """Parse a document's change log into amendment + rationale objects.

    Accepts all three authored shapes required by SBC-00-ADDENDUM-D §2.3:
    block (§2.1), compact (§2.2), and the legacy table/bullet forms.
    """
    objects: list[SpecObject] = []
    in_log = False
    log_depth = 0

    pending = None  # in-flight block-form entry
    pending_line = 0
    rationale: list[str] = []
    in_rationale = False
    in_fence = False
    # The compact form is identified by its column header, never by counting
    # cells: legacy summaries contain literal '|' characters, which inflates
    # the cell count and made a legacy row parse as compact with a garbage
    # change_kind (observed at TODO-SSP-04-PUBLICATION-RENDERING.md:607).
    compact_table = False

    def flush():
        nonlocal pending, rationale, in_rationale
        if not pending:
            return
        text = "\n".join(rationale).strip()
        pending["has_rationale"] = bool(text)
        obj = _amendment(doc, family, pending_line, "block", **pending)
        objects.append(obj)
        if text:
            objects.append(
                SpecObject(
                    obj_id=f"{doc}#{pending['rev']}-rationale",
                    kind="rationale",
                    family=family,
                    doc=doc,
                    line=pending_line,
                    text=text,
                    attrs={"motivates": pending.get("touches") or []},
                )
            )
        pending, rationale, in_rationale = None, [], False

    for lineno, line in enumerate(lines, start=1):
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            # Documents that teach the shape contain example entries; a fenced
            # block is illustration, never corpus content.
            continue

        head = RE_HEADING.match(line)
        if head:
            depth = len(head.group(1))
            if RE_CL_SECTION.match(line):
                flush()
                in_log, log_depth, compact_table = True, depth, False
                continue
            if in_log and depth <= log_depth:
                flush()
                in_log = False
                continue
        if not in_log:
            continue

        bh = RE_CL_BLOCK_HEAD.match(line)
        if bh:
            flush()
            pending_line = lineno
            pending = {
                "rev": bh.group("rev"),
                "date": bh.group("date"),
                "status": bh.group("status").strip().lower(),
                "author": None,
                "change_kind": None,
                "touches": None,
                "commits": None,
                "summary": None,
            }
            continue

        if pending is not None:
            if RE_CL_RATIONALE_HEAD.match(line):
                in_rationale = True
                continue
            fm = RE_CL_FIELD.match(line)
            if fm and not in_rationale:
                key = fm.group(1).lower().replace(" ", "_")
                val = fm.group(2).strip()
                if key in {"touches", "commits"}:
                    pending[key] = _split_list(val)
                elif key == "change_kind":
                    pending[key] = val.strip().lower()
                else:
                    pending[key] = val
                continue
            if in_rationale:
                rationale.append(line)
            continue

        if (
            line.lstrip().startswith("|")
            and "change kind" in line.lower()
            # A header row never opens with a revision. Without this guard a
            # legacy-table *data* row whose summary prose says "Change kind:
            # semantic" is read as a column header and skipped entirely — and
            # that phrasing is exactly the convention SBC-00-CONCEPTS adopted,
            # which silently dropped its own 0.8.0 and 0.9.0 ratifications.
            and not RE_CL_TABLE_ROW.match(line)
            # A header row never opens with a revision. Without this guard a
            # legacy-table *data* row whose summary prose says "Change kind:
            # semantic" is read as a column header and skipped entirely — and
            # that phrasing is exactly the convention SBC-00-CONCEPTS adopted,
            # which silently dropped its own 0.8.0 and 0.9.0 ratifications.
        ):
            compact_table = True  # header row declares the §2.2 columns
            continue

        tm = RE_CL_TABLE_ROW.match(line)
        if tm:
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            if compact_table and len(cells) >= 7:  # compact form, ADDENDUM-D §2.2
                objects.append(
                    _amendment(
                        doc, family, lineno, "compact",
                        rev=cells[0], date=cells[1], author=cells[2],
                        status=cells[3].strip("* ").lower(),
                        change_kind=cells[4].strip("* ").lower(),
                        touches=_split_list(cells[5]),
                        commits=None,
                        summary=cells[6],
                        has_rationale=False,
                    )
                )
            else:  # legacy table: no change-kind or touches column exists
                objects.append(
                    _amendment(
                        doc, family, lineno, "legacy-table",
                        rev=cells[0], date=cells[1],
                        author=cells[2] if len(cells) > 2 else None,
                        status=(cells[3].strip("* ").lower() if len(cells) > 3 else None),
                        change_kind=None, touches=None, commits=None,
                        summary=(cells[4] if len(cells) > 4 else ""),
                        has_rationale=False,
                    )
                )
            continue

        bm = RE_CL_BULLET_ROW.match(line)
        if bm:
            status = RE_STATUS_WORD.findall(line)
            objects.append(
                _amendment(
                    doc, family, lineno, "legacy-bullet",
                    rev=None, date=bm.group(1), author=None,
                    status=(status[0].strip().rstrip(".").lower() if status else None),
                    change_kind=None, touches=None, commits=None,
                    summary=line.strip("- ").strip(),
                    has_rationale=False,
                )
            )

    flush()

    # D-C3: ordering derives from the declared date, never file position.
    # Consumers get the rank as an attribute so nobody re-derives it (and
    # gets it differently); the index itself stays sorted by position so it
    # remains deterministic under SBC-INV-20.
    amendments = [o for o in objects if o.kind == "amendment"]
    dated = [o for o in amendments if o.attrs.get("date")]
    for rank, obj in enumerate(
        sorted(dated, key=lambda o: (o.attrs["date"], o.line)), start=1
    ):
        obj.attrs["chronological_rank"] = rank
    file_order = [o.attrs.get("date") for o in dated]
    for obj in amendments:
        obj.attrs["doc_entries_out_of_order"] = file_order != sorted(file_order)

    return objects


def parse_document(path: pathlib.Path, repo_root: pathlib.Path) -> tuple[Document, list, list]:
    # PurePosixPath, not the platform-native flavour: every ``str(rel)`` below
    # is emitted into the committed index, and SBC-INV-20 requires that index
    # to regenerate to a no-op.  A native Path stringifies with backslashes on
    # Windows, which rewrites every source path in the corpus and turns the
    # no-op check into a whole-corpus diff.  ``.parts`` is unaffected, so
    # ``family_of`` behaves identically.
    rel = pathlib.PurePosixPath(path.relative_to(repo_root).as_posix())
    doc = Document(
        path=str(rel),
        family=family_of(rel),
        is_concepts="CONCEPTS" in path.name.upper(),
    )
    objects: list[SpecObject] = []
    citations: list[Citation] = []

    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return doc, objects, citations

    section = None
    in_header = True
    cl_table = cl_bullet = 0
    defined_here: set[str] = set()

    for lineno, line in enumerate(lines, start=1):
        # -- headings drive section state -------------------------------
        m = RE_HEADING.match(line)
        if m:
            in_header = False
            title = m.group(2).strip()
            num = None
            sm = RE_SECTION_NUM.match(title.lstrip("#").strip())
            if sm:
                num = int(sm.group(1))
            else:
                low = title.lower()
                for hint, hint_num in SECTION_HINTS.items():
                    if hint in low:
                        num = hint_num
                        break
            if num is not None:
                section = num
                doc.sections.add(num)
            elif m.group(1) == "##":
                section = None

        # -- document header attributes ---------------------------------
        if in_header:
            hm = RE_HEADER_ATTR.match(line)
            if hm:
                doc.header[hm.group(1).strip().lower()] = hm.group(2).strip()

        # -- change-log shape detection ---------------------------------
        if section == 15:
            if RE_CL_TABLE_ROW.match(line):
                cl_table += 1
            elif RE_CL_BULLET_ROW.match(line):
                cl_bullet += 1

        # -- invariant definitions (four shapes) -------------------------
        inv_id = statement = verified = title = None
        shape = None
        dm = RE_INV_DEF_ROW.match(line)
        if dm:
            inv_id = dm.group(1)
            cells = [c.strip() for c in dm.group(2).split("|")]
            statement = cells[0] if cells else ""
            verified = cells[1] if len(cells) > 1 else ""
            title = ""
            shape = "table"
        else:
            bm = RE_INV_DEF_BULLET.match(line)
            if bm:
                inv_id, title, statement = bm.group(1), bm.group(2).strip(), bm.group(3).strip()
                verified = ""
                shape = "bullet"
            else:
                hm2 = RE_INV_DEF_HEADING.match(line)
                if hm2:
                    inv_id, title, statement = hm2.group(1), hm2.group(2).strip(), ""
                    verified = ""
                    shape = "heading"

        if inv_id:
            defined_here.add(inv_id)
            objects.append(
                SpecObject(
                    obj_id=inv_id,
                    kind="invariant",
                    family=doc.family,
                    doc=str(rel),
                    line=lineno,
                    text=statement,
                    attrs={
                        "title": title,
                        "def_shape": shape,
                        "verified_by": verified,
                        "has_verification": bool(verified and verified not in {"", "—", "-"}),
                        "normative_keywords": sorted(set(RFC2119.findall(statement))),
                        "doc_status": doc.header.get("status", "").strip(),
                    },
                )
            )

        # -- errata entries ---------------------------------------------
        em = RE_ERRATA_DEF.match(line)
        if em:
            eid = em.group(1)
            scoped = f"{doc.family}:{eid}"
            defined_here.add(eid)
            objects.append(
                SpecObject(
                    obj_id=scoped,
                    kind="errata",
                    family=doc.family,
                    doc=str(rel),
                    line=lineno,
                    text=em.group(2).strip(),
                )
            )

        # -- glossary terms ----------------------------------------------
        if section == 3:
            gm = RE_GLOSSARY_ROW.match(line)
            if gm:
                term = gm.group(1).strip()
                low = term.lower()
                if low not in AUTHORITY_AXES and not term.startswith("Term"):
                    objects.append(
                        SpecObject(
                            obj_id=f"{doc.family}:term:{low}",
                            kind="term",
                            family=doc.family,
                            doc=str(rel),
                            line=lineno,
                            text=gm.group(2).strip()[:400],
                            attrs={"relationship": gm.group(3).strip()[:200], "term": term},
                        )
                    )

        # -- acceptance gates --------------------------------------------
        if section == 12:
            am = RE_ACCEPTANCE_GATE.match(line)
            if am:
                objects.append(
                    SpecObject(
                        obj_id=f"{rel}:gate:{lineno}",
                        kind="gate",
                        family=doc.family,
                        doc=str(rel),
                        line=lineno,
                        text=am.group(2).strip()[:400],
                        attrs={
                            "checked": am.group(1).lower() == "x",
                            "cited_ids": sorted(
                                {mm.group(0) for mm in RE_INVARIANT.finditer(am.group(2))}
                            ),
                        },
                    )
                )

        # -- non-goals ----------------------------------------------------
        if section == 11:
            nm = RE_NONGOAL.match(line)
            if nm:
                objects.append(
                    SpecObject(
                        obj_id=f"{rel}:nongoal:{nm.group(1)}",
                        kind="nongoal",
                        family=doc.family,
                        doc=str(rel),
                        line=lineno,
                        text=(nm.group(2) + " " + nm.group(3)).strip()[:400],
                    )
                )

        # -- citations (every id mention, anywhere) -----------------------
        for cm in RE_INVARIANT.finditer(line):
            citations.append(
                Citation(
                    obj_id=cm.group(0),
                    doc=str(rel),
                    line=lineno,
                    family=doc.family,
                    is_definition=(inv_id is not None and cm.group(0) == inv_id),
                )
            )
        for pm in RE_PCDN.finditer(line):
            citations.append(
                Citation(obj_id=pm.group(0), doc=str(rel), line=lineno, family=doc.family)
            )
        for qm in RE_EOQ.finditer(line):
            citations.append(
                Citation(obj_id=qm.group(0), doc=str(rel), line=lineno, family=doc.family)
            )

    # Change log — all three authored shapes (ADDENDUM-D §2.3, D-C1).
    cl_objects = parse_changelog_entries(lines, str(rel), doc.family)
    objects.extend(cl_objects)
    shapes = {o.attrs["shape"] for o in cl_objects if o.kind == "amendment"}
    amendments = [o for o in cl_objects if o.kind == "amendment"]
    if len(shapes) > 1:
        doc.changelog_shape = "mixed"
    elif shapes:
        doc.changelog_shape = next(iter(shapes))
    elif 15 in doc.sections or 9 in doc.sections:
        doc.changelog_shape = "unparsed" if RE_CL_SECTION_PRESENT(lines) else "none"
    doc.changelog_entries = len(amendments)

    return doc, objects, citations


def RE_CL_SECTION_PRESENT(lines) -> bool:  # noqa: N802 - reads as a predicate
    return any(RE_CL_SECTION.match(ln) for ln in lines)


def scan(repo_root: pathlib.Path) -> dict:
    docs: list[Document] = []
    objects: list[SpecObject] = []
    citations: list[Citation] = []

    for scan_dir in SCAN_DIRS:
        base = repo_root / scan_dir
        if not base.exists():
            continue
        for path in sorted(base.rglob("*.md")):
            if EXCLUDE_PARTS & set(path.parts):
                continue
            d, o, c = parse_document(path, repo_root)
            docs.append(d)
            objects.extend(o)
            citations.extend(c)

    return {"docs": docs, "objects": objects, "citations": citations}


# --------------------------------------------------------------------------
# Findings
# --------------------------------------------------------------------------


def _vacuity_guarded(violations: int, eligible: int, total: int, note: str) -> dict:
    """Report a conformance count together with whether it could have failed.

    A check whose subject is absent passes trivially and is indistinguishable
    from a check that genuinely passed.  Here: SBC-INV-17 can only be violated
    by an amendment that declares a ChangeKind, so while almost none do, a
    zero is evidence of an unpopulated field — not of compliance.
    """
    out = {"violations": violations, "eligible": eligible, "of_total": total, "note": note}
    if eligible == 0:
        out["VACUOUS"] = "no eligible subjects — this check cannot currently fail"
    elif total and eligible / total < 0.05:
        out["VACUOUS"] = (
            f"only {eligible}/{total} amendments declare a ChangeKind; "
            "a low violation count reflects an unpopulated field, not compliance"
        )
    return out


def report(data: dict) -> dict:
    docs: list[Document] = data["docs"]
    objects: list[SpecObject] = data["objects"]
    citations: list[Citation] = data["citations"]

    invariants = [o for o in objects if o.kind == "invariant"]
    terms = [o for o in objects if o.kind == "term"]
    gates = [o for o in objects if o.kind == "gate"]
    errata = [o for o in objects if o.kind == "errata"]
    nongoals = [o for o in objects if o.kind == "nongoal"]

    concepts = [d for d in docs if d.is_concepts]

    # F1 — change-log shape drift across concepts docs.
    shape_counts = collections.Counter(d.changelog_shape for d in concepts)

    # F2 — invariants with no verification surface.
    unverified = [o for o in invariants if not o.attrs.get("has_verification")]

    # F3 — invariant definitions that are duplicated (same id, two def sites).
    by_id: dict[str, list] = collections.defaultdict(list)
    for o in invariants:
        by_id[o.obj_id].append(o)
    duplicated = {
        k: [f"{o.doc}:{o.line} [{o.attrs.get('doc_status') or 'no-status'}]" for o in v]
        for k, v in by_id.items()
        if len(v) > 1
    }

    # F10 — definition-shape drift: one semantic act, four renderings.
    shape_dist = collections.Counter(o.attrs.get("def_shape") for o in invariants)

    # F11 — definitions living in docs whose header status is SUPERSEDED.
    superseded_defs = [
        f"{o.obj_id} ({o.doc})"
        for o in invariants
        if "supersede" in (o.attrs.get("doc_status") or "").lower()
    ]

    # F4 — orphan citations: an id cited somewhere but defined nowhere.
    defined_ids = {o.obj_id for o in invariants}
    cited_inv = collections.Counter(
        c.obj_id
        for c in citations
        if RE_INVARIANT.fullmatch(c.obj_id) and not is_example_id(c.obj_id)
    )
    orphans = {k: v for k, v in cited_inv.items() if k not in defined_ids}

    # F5 — cross-family term collisions.
    by_term: dict[str, set] = collections.defaultdict(set)
    for t in terms:
        by_term[t.attrs["term"].strip().lower()].add(t.family)
    collisions = {k: sorted(v) for k, v in by_term.items() if len(v) > 1}

    # F6 — invariants with no RFC2119 keyword in their statement.
    keywordless = [o for o in invariants if not o.attrs.get("normative_keywords")]

    # F7 — acceptance gates, and how many cite an invariant id.
    gates_citing = [g for g in gates if g.attrs.get("cited_ids")]

    # F8 — citation fan-out per invariant (the suspect-blast-radius proxy).
    fanout = collections.Counter(
        c.obj_id for c in citations if c.obj_id in defined_ids and not c.is_definition
    )

    # F9 — load-bearing section coverage (SBC-INV-9: §0, §3/§4, §10, §12, §15).
    missing_sections = []
    for d in concepts:
        need = {0, 10, 12, 15}
        missing = sorted(need - d.sections)
        if not (3 in d.sections or 4 in d.sections):
            missing.append(34)
        if missing:
            missing_sections.append({"doc": d.path, "missing": missing})

    return {
        "totals": {
            "documents": len(docs),
            "concepts_docs": len(concepts),
            "objects": len(objects),
            "invariants": len(invariants),
            "terms": len(terms),
            "gates": len(gates),
            "errata": len(errata),
            "nongoals": len(nongoals),
            "citations": len(citations),
            "families": len({d.family for d in docs}),
        },
        "F1_changelog_shape": dict(shape_counts),
        "F2_unverified_invariants": {
            "count": len(unverified),
            "pct": round(100 * len(unverified) / max(1, len(invariants)), 1),
            "sample": [f"{o.obj_id} ({o.doc})" for o in unverified[:10]],
        },
        "F3_duplicate_definitions": duplicated,
        "F4_orphan_citations": {
            "count": len(orphans),
            "top": sorted(orphans.items(), key=lambda kv: -kv[1])[:12],
        },
        "F5_term_collisions": {
            "count": len(collisions),
            "top": sorted(collisions.items(), key=lambda kv: -len(kv[1]))[:15],
        },
        "F6_keywordless_invariants": {
            "count": len(keywordless),
            "pct": round(100 * len(keywordless) / max(1, len(invariants)), 1),
        },
        "F7_acceptance_gates": {
            "total": len(gates),
            "citing_an_invariant": len(gates_citing),
            "pct": round(100 * len(gates_citing) / max(1, len(gates)), 1),
        },
        "F8_top_blast_radius": fanout.most_common(12),
        "F9_missing_load_bearing_sections": {
            "count": len(missing_sections),
            "sample": missing_sections[:10],
        },
        "F10_definition_shape_drift": dict(shape_dist),
        "F13_amendment_shapes": dict(
            collections.Counter(
                o.attrs["shape"] for o in objects if o.kind == "amendment"
            )
        ),
        "F14_amendments_without_change_kind": {
            "count": sum(
                1
                for o in objects
                if o.kind == "amendment" and not o.attrs.get("change_kind")
            ),
            "note": "no ChangeKind declared -> suspicion cannot be typed (SBC-INV-19)",
        },
        "F15_inv17_rationale_gap": _vacuity_guarded(
            violations=sum(
                1
                for o in objects
                if o.kind == "amendment"
                and o.attrs.get("rationale_required")
                and not o.attrs.get("has_rationale")
            ),
            eligible=sum(
                1 for o in objects if o.kind == "amendment" and o.attrs.get("rationale_required")
            ),
            total=sum(1 for o in objects if o.kind == "amendment"),
            note="semantic/scope/retirement amendments lacking a rationale (SBC-INV-17)",
        ),
        "F17_out_of_order_changelogs": {
            "count": len(
                {
                    o.doc
                    for o in objects
                    if o.kind == "amendment" and o.attrs.get("doc_entries_out_of_order")
                }
            ),
            "docs": sorted(
                {
                    o.doc
                    for o in objects
                    if o.kind == "amendment" and o.attrs.get("doc_entries_out_of_order")
                }
            )[:10],
            "note": "file position diverges from declared date (ADDENDUM-D §7.2 D-C3)",
        },
        "F16_amendments_naming_touches": {
            "count": sum(
                1 for o in objects if o.kind == "amendment" and o.attrs.get("touches")
            ),
            "of_total": sum(1 for o in objects if o.kind == "amendment"),
        },
        "F11_definitions_in_superseded_docs": {
            "count": len(superseded_defs),
            "sample": superseded_defs[:10],
        },
    }


# --------------------------------------------------------------------------
# Change-log audit (PCDN-SBC-00-C-003)
# --------------------------------------------------------------------------

RE_ANY_ID = re.compile(
    rf"\b(?:{_INV_ID}|ERRATA-\d+|PCDN-[A-Z0-9-]+|EOQ-\d+-ERRATA-\d+)\b"
)
RE_SHA = re.compile(r"\b[0-9a-f]{7,40}\b")
RE_STATUS_WORD = re.compile(
    r"\*\*([^*]{0,40}?(?:Ratified|Amended|DRAFT|Draft|Drafted|Execution|Superseded)[^*]{0,40}?)\*\*"
)
# Length threshold above which an entry is structurally carrying more than a
# delta list.  A lexical "rationale marker" proxy was tried first and rejected:
# it scored TODO-MCAD-00 rev 0.4.0 — 2,147 characters of sustained design
# argument — at zero, so it measured the regex rather than the corpus.  Length
# is instrument-independent.  Per ADDENDUM-C §4.2 this attribute is `inferred`.
LONG_ENTRY_CHARS = 800


def audit_changelogs(repo_root: pathlib.Path) -> dict:
    """Measure what §15 entries actually carry, in both authored shapes."""
    entries = []

    for scan_dir in SCAN_DIRS:
        base = repo_root / scan_dir
        if not base.exists():
            continue
        for path in sorted(base.rglob("*.md")):
            if EXCLUDE_PARTS & set(path.parts) or "CONCEPTS" not in path.name.upper():
                continue
            rel = path.relative_to(repo_root).as_posix()  # posix: see parse_document
            lines = path.read_text(encoding="utf-8", errors="replace").splitlines()

            section = None
            for line in lines:
                m = RE_HEADING.match(line)
                if m:
                    title = m.group(2).strip()
                    sm = RE_SECTION_NUM.match(title)
                    if sm:
                        section = int(sm.group(1))
                    elif "change log" in title.lower():
                        section = 15
                    elif m.group(1) == "##":
                        section = None
                if section != 15:
                    continue

                shape = rev = date = author = None
                body = ""
                tm = RE_CL_TABLE_ROW.match(line)
                if tm:
                    cells = [c.strip() for c in line.strip().strip("|").split("|")]
                    shape, rev, date = "table", tm.group(1), tm.group(2)
                    author = cells[2] if len(cells) > 2 else ""
                    # Status is its own column in the table shape; the bullet
                    # shape inlines it into the prose.  Scan both for status
                    # so the two shapes are measured on equal terms.
                    body = " | ".join(cells[3:]) if len(cells) > 3 else ""
                else:
                    bm = RE_CL_BULLET_ROW.match(line)
                    if bm:
                        shape, date = "bullet", bm.group(1)
                        body = line
                if not shape:
                    continue

                status = RE_STATUS_WORD.findall(body)
                entries.append(
                    {
                        "doc": rel,
                        "shape": shape,
                        "has_rev": bool(rev),
                        "has_author": bool(author and author not in {"", "—", "-"}),
                        "date": date,
                        "status": status[0].strip() if status else None,
                        "chars": len(body),
                        "ids_named": sorted(set(RE_ANY_ID.findall(body))),
                        "shas_named": len(RE_SHA.findall(body)),
                        "is_long": len(body) >= LONG_ENTRY_CHARS,
                    }
                )

    def stats(rows, key):
        vals = sorted(r[key] for r in rows)
        if not vals:
            return {}
        return {
            "n": len(vals),
            "median": vals[len(vals) // 2],
            "p90": vals[int(len(vals) * 0.9)],
            "max": vals[-1],
        }

    out = {"total_entries": len(entries)}
    for shape in ("table", "bullet"):
        rows = [e for e in entries if e["shape"] == shape]
        if not rows:
            continue
        out[shape] = {
            "entries": len(rows),
            "docs": len({r["doc"] for r in rows}),
            "carries_rev_pct": round(100 * sum(r["has_rev"] for r in rows) / len(rows), 1),
            "carries_author_pct": round(100 * sum(r["has_author"] for r in rows) / len(rows), 1),
            "carries_status_pct": round(100 * sum(bool(r["status"]) for r in rows) / len(rows), 1),
            "names_an_id_pct": round(100 * sum(bool(r["ids_named"]) for r in rows) / len(rows), 1),
            "names_a_sha_pct": round(100 * sum(bool(r["shas_named"]) for r in rows) / len(rows), 1),
            "long_entry_pct": round(100 * sum(r["is_long"] for r in rows) / len(rows), 1),
            "entry_chars": stats(rows, "chars"),
        }

    # Chronological ordering violations (append-drift).
    by_doc: dict[str, list] = collections.defaultdict(list)
    for e in entries:
        if e["date"]:
            by_doc[e["doc"]].append(e["date"])
    unordered = [d for d, ds in by_doc.items() if ds != sorted(ds)]
    out["docs_with_out_of_order_entries"] = {
        "count": len(unordered),
        "sample": unordered[:6],
    }

    # Distinct status vocabularies in use — an unfrozen enum in the wild.
    out["status_vocabulary"] = collections.Counter(
        e["status"] for e in entries if e["status"]
    ).most_common(14)

    return out


# --------------------------------------------------------------------------
# Touches: backfill proposal
# --------------------------------------------------------------------------

# Verbs that indicate an amendment ACTED ON the nearby id, as opposed to
# merely mentioning it.  Deliberately conservative: a proposal that has to be
# hand-checked anyway is worth more when it is precise than when it is broad.
RE_ACTED_ON = re.compile(
    r"\b(amended|added|adds|clarified|introduces?|new|frozen|freezes|"
    r"registers?|updated|renamed|retired|withdrawn|scoped|expanded|"
    r"replaced|supersedes?|removed|dropped)\b",
    re.IGNORECASE,
)
# Explicit disclaimers.  MCAD-00 0.3.0 says "INV-MCAD-5 not amended", and a
# naive extractor would record the exact opposite of what the author wrote.
RE_NOT_ACTED = re.compile(
    r"\b(not amended|unmodified|unchanged|untouched|no change|carried over|"
    r"did not (?:need|change)|need(?:s|ed)? no (?:amendment|change)|"
    r"no amendment|without modification|left alone|survive[sd]? unmodified)\b",
    re.IGNORECASE,
)


def propose_touches(objects) -> list:
    """Derive a Touches: proposal from ids the author already named.

    This never invents a relationship. It reports ids that appear in an
    amendment's own prose alongside an action verb, with the evidence
    sentence, so a human ratifies rather than trusts. Output is a proposal:
    per ADDENDUM-C §4.2 anything landed from it is `inferred`, not
    `declared`, until an author confirms it.
    """
    proposals = []
    for obj in objects:
        if obj.kind != "amendment" or obj.attrs.get("touches"):
            continue
        text = obj.text or ""
        if not text:
            continue
        hits: dict[str, str] = {}
        for sentence in re.split(r"(?<=[.;])\s+", text):
            ids = {
                m.group(0)
                for m in RE_INVARIANT.finditer(sentence)
                if not is_example_id(m.group(0))
            }
            if not ids:
                continue
            if RE_NOT_ACTED.search(sentence):
                continue
            if not RE_ACTED_ON.search(sentence):
                continue
            for i in sorted(ids):
                hits.setdefault(i, sentence.strip()[:180])
        if hits:
            proposals.append(
                {
                    "doc": obj.doc,
                    "line": obj.line,
                    "rev": obj.attrs.get("rev"),
                    "date": obj.attrs.get("date"),
                    "proposed_touches": sorted(hits),
                    "evidence": hits,
                    "provenance": "inferred",
                }
            )
    return proposals


# --------------------------------------------------------------------------
# Committed object index (SBC-INV-20)
# --------------------------------------------------------------------------

# Bump when the emitted shape changes.  A parser change that alters output
# MUST show up as a diff — that is the entire point of committing the index
# rather than reconstructing history by replaying a current parser over past
# revisions (SBC-00-ADDENDUM-C §7).
INDEX_SCHEMA_VERSION = 2
DEFAULT_INDEX_DIR = "docs/spec-index/index"


def build_index(data: dict) -> dict:
    """Build the deterministic per-family index.

    Determinism is the load-bearing property: same corpus -> byte-identical
    output, so `--check-index` can assert regeneration is a no-op.  Nothing
    time-, path-, or environment-dependent may enter this structure.

    In particular the index does NOT record the commit it was generated at.
    Committing the index changes HEAD, so an embedded HEAD SHA would make the
    no-op check unsatisfiable by construction.  Git already knows which commit
    an index belongs to; baseline stamps are applied at handoff, not here.
    """
    per_family: dict[str, list] = collections.defaultdict(list)
    for obj in data["objects"]:
        per_family[obj.family].append(obj.to_dict())

    # Edges. A `cites` edge per non-definition citation, `defines` per
    # definition site. Typed edge kinds beyond these are declared by authors
    # (ADDENDUM-C §5) and carry kind_provenance accordingly; everything the
    # parser derives is `inferred`.
    edges_by_family: dict[str, list] = collections.defaultdict(list)
    for c in data["citations"]:
        edges_by_family[c.family].append(
            {
                "edge_type": "defines" if c.is_definition else "cites",
                "source": c.doc,
                "target": c.obj_id,
                "line": c.line,
                "kind_provenance": "inferred",
            }
        )

    out: dict[str, dict] = {}
    for family in set(per_family) | set(edges_by_family):
        objects = per_family.get(family, [])
        objects.sort(key=lambda o: (o["doc"], o["line"], o["kind"], o["obj_id"]))
        edges = edges_by_family.get(family, [])
        edges.sort(key=lambda e: (e["source"], e["line"], e["edge_type"], e["target"]))
        out[family] = {
            "schema_version": INDEX_SCHEMA_VERSION,
            "family": family,
            "object_count": len(objects),
            "edge_count": len(edges),
            "kind_counts": dict(sorted(collections.Counter(o["kind"] for o in objects).items())),
            "objects": objects,
            "edges": edges,
        }

    out["_manifest"] = {
        "schema_version": INDEX_SCHEMA_VERSION,
        "families": sorted(per_family),
        "family_counts": {f: len(per_family[f]) for f in sorted(per_family)},
        "total_objects": sum(len(v) for v in per_family.values()),
        "total_edges": sum(len(v) for v in edges_by_family.values()),
    }
    return out


def _render(payload: dict) -> str:
    return json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True) + "\n"


def emit_index(index: dict, index_dir: pathlib.Path) -> list:
    index_dir.mkdir(parents=True, exist_ok=True)
    written = []
    for name, payload in sorted(index.items()):
        path = index_dir / f"{name}.json"
        # newline="\n" is explicit: the default translates to CRLF on Windows,
        # which contradicts .gitattributes `* text=auto eol=lf` and leaves the
        # working tree differing from the committed blob.
        path.write_text(_render(payload), encoding="utf-8", newline="\n")
        written.append(path.name)
    # Remove files for families that no longer exist, so the directory is a
    # faithful projection rather than an accumulation.
    expected = {f"{n}.json" for n in index}
    for stale in sorted(p for p in index_dir.glob("*.json") if p.name not in expected):
        stale.unlink()
        written.append(f"-{stale.name}")
    return written


def check_index(index: dict, index_dir: pathlib.Path) -> list:
    """Return a list of drift descriptions; empty means regeneration is a no-op."""
    drift = []
    if not index_dir.exists():
        return [f"index directory {index_dir} does not exist — run --emit-index"]

    on_disk = {p.name for p in index_dir.glob("*.json")}
    expected = {f"{n}.json" for n in index}
    for missing in sorted(expected - on_disk):
        drift.append(f"missing: {missing}")
    for extra in sorted(on_disk - expected):
        drift.append(f"stale: {extra}")

    for name, payload in sorted(index.items()):
        path = index_dir / f"{name}.json"
        if not path.exists():
            continue
        current = path.read_text(encoding="utf-8")
        regenerated = _render(payload)
        if current != regenerated:
            try:
                old = json.loads(current)
            except json.JSONDecodeError:
                drift.append(f"changed: {name}.json (unparseable on disk)")
                continue
            # `_manifest` has no object_count; report whatever the file carries.
            new_n = payload.get("object_count", payload.get("total_objects"))
            old_n = old.get("object_count", old.get("total_objects"))
            if new_n is not None and old_n is not None and new_n != old_n:
                drift.append(f"changed: {name}.json (objects {old_n} -> {new_n})")
            else:
                drift.append(f"changed: {name}.json (content differs, same object count)")
    return drift


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".", help="repo root")
    ap.add_argument("--json", help="write full object/citation dump here")
    ap.add_argument(
        "--changelog-audit",
        action="store_true",
        help="audit §15 change-log entries (PCDN-SBC-00-C-003)",
    )
    ap.add_argument(
        "--emit-index", action="store_true", help="write the committed object index"
    )
    ap.add_argument(
        "--check-index",
        action="store_true",
        help="verify regeneration is a no-op; exit 1 on drift (SBC-INV-20)",
    )
    ap.add_argument("--index-dir", default=DEFAULT_INDEX_DIR)
    ap.add_argument(
        "--propose-touches",
        metavar="FAMILY",
        nargs="?",
        const="*",
        help="propose Touches: values derived from amendment prose (never writes)",
    )
    args = ap.parse_args()

    repo_root = pathlib.Path(args.root).resolve()

    if args.changelog_audit:
        print(json.dumps(audit_changelogs(repo_root), indent=2, ensure_ascii=False))
        return 0

    data = scan(repo_root)

    if args.propose_touches:
        objs = data["objects"]
        if args.propose_touches != "*":
            objs = [o for o in objs if o.family == args.propose_touches]
        proposals = propose_touches(objs)
        total = sum(1 for o in objs if o.kind == "amendment")
        print(
            json.dumps(
                {
                    "family": args.propose_touches,
                    "amendments_scanned": total,
                    "with_a_proposal": len(proposals),
                    "note": (
                        "PROPOSAL ONLY — nothing written. Each entry cites the "
                        "sentence it was derived from; anything landed is "
                        "`inferred` until an author confirms it."
                    ),
                    "proposals": proposals,
                },
                indent=2,
                ensure_ascii=False,
            )
        )
        return 0

    if args.emit_index or args.check_index:
        index = build_index(data)
        index_dir = repo_root / args.index_dir
        if args.emit_index:
            written = emit_index(index, index_dir)
            print(f"wrote {len(written)} file(s) to {args.index_dir}")
            print(f"total objects: {index['_manifest']['total_objects']}")
            return 0
        drift = check_index(index, index_dir)
        if drift:
            print("INDEX DRIFT — regeneration is not a no-op (SBC-INV-20):")
            for d in drift:
                print(f"  {d}")
            print("\nRun: python3 scripts/specidx/scan.py --emit-index")
            return 1
        print(f"index clean: {index['_manifest']['total_objects']} objects, "
              f"{len(index['_manifest']['families'])} families")
        return 0

    findings = report(data)

    print(json.dumps(findings, indent=2, ensure_ascii=False))

    if args.json:
        dump = {
            "documents": [dataclasses.asdict(d) | {"sections": sorted(d.sections)} for d in data["docs"]],
            "objects": [o.to_dict() for o in data["objects"]],
            "citations": [dataclasses.asdict(c) for c in data["citations"]],
            "findings": findings,
        }
        pathlib.Path(args.json).write_text(
            json.dumps(dump, indent=2, ensure_ascii=False),
            encoding="utf-8",
            newline="\n",
        )
        print(f"\n[wrote {args.json}]", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
