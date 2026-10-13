#!/usr/bin/env python3
"""Derived suspicion over the rlvgl spec corpus.

Conformance target: SBC-00-CONCEPTS SBC-INV-19; SBC-00-ADDENDUM-C §8.

An object is *suspect* when something it depends on changed after the object
last acknowledged it.  Suspicion is derived — a function of the citation graph
and version-control history — and is never authored.  Clearings are the one
non-derivable fact and live in commit trailers, not here (ADDENDUM-C §8.3).

Division of labour: `scan.py` maps the corpus to objects and never touches
git; this module joins those objects to git history.  Keeping the boundary
means the index stays reproducible from the working tree alone.

Usage:
    python3 scripts/specidx/suspect.py [--root .] [--json OUT]
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import re
import statistics
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))

import scan  # noqa: E402

# ADDENDUM-C §8.2 — which change kinds propagate, and along which edges.
PROPAGATION = {
    "editorial": set(),
    "clarification": {"refines"},
    "semantic": {"*"},
    "scope": {"*"},
    "retirement": {"*"},
}

RE_CLEARS = re.compile(
    r"^\s*Clears-Suspect:\s*(?P<id>[A-Za-z0-9_-]+)"
    r"(?:@(?P<rev>[0-9A-Za-z.]+))?\s*(?:->|→)\s*(?P<target>\S+)",
    re.MULTILINE,
)


def _git(root: pathlib.Path, *args: str) -> str:
    # encoding/errors are explicit: the corpus is UTF-8, but text=True decodes
    # with the locale encoding, which is cp1252 on Windows and raises inside
    # subprocess's reader thread, surfacing as stdout=None.
    return subprocess.run(
        ["git", *args],
        cwd=root,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    ).stdout


def last_touched(root: pathlib.Path) -> dict:
    """Map repo-relative path -> unix timestamp of its most recent commit.

    One git pass rather than one per file; at 642 documents the per-file form
    costs ~640 subprocess spawns.
    """
    out = _git(root, "log", "--format=C%ct", "--name-only", "--", "docs")
    seen: dict[str, int] = {}
    ts = 0
    for line in out.splitlines():
        if line.startswith("C") and line[1:].isdigit():
            ts = int(line[1:])
        elif line.strip() and ts:
            seen.setdefault(line.strip(), ts)
    return seen


def clearings(root: pathlib.Path) -> list:
    """Parse `Clears-Suspect:` trailers from history (ADDENDUM-C §8.3)."""
    out = _git(root, "log", "--format=%H%x00%ct%x00%B%x01")
    found = []
    for entry in out.split("\x01"):
        if not entry.strip():
            continue
        parts = entry.strip().split("\x00", 2)
        if len(parts) < 3:
            continue
        sha, ts, body = parts
        for m in RE_CLEARS.finditer(body):
            found.append(
                {
                    "sha": sha[:9],
                    "ts": int(ts),
                    "id": m.group("id"),
                    "rev": m.group("rev"),
                    "target": m.group("target"),
                }
            )
    return found


def compute(root: pathlib.Path) -> dict:
    """Join the corpus index to git history.  I/O only; logic is in derive()."""
    data = scan.scan(root)
    return derive(
        objects=data["objects"],
        citations=data["citations"],
        touched_ts=last_touched(root),
        cleared=clearings(root),
    )


def derive(objects, citations, touched_ts, cleared) -> dict:
    """Pure suspicion derivation — no I/O, so it can be exercised directly."""
    amendments = [o for o in objects if o.kind == "amendment"]
    cleared_keys = {(c["id"], c["target"]) for c in cleared}

    # Citation sites per object id, excluding the definition site.
    sites: dict[str, list] = collections.defaultdict(list)
    definitions = {o.obj_id: o.doc for o in objects if o.kind == "invariant"}
    for c in citations:
        if c.obj_id in definitions and not c.is_definition:
            sites[c.obj_id].append(c)

    suspects = []
    untypeable = []
    non_propagating = 0

    for am in amendments:
        kind = am.attrs.get("change_kind")
        touches = am.attrs.get("touches")

        if not kind:
            # Honest bucket.  Defaulting to `semantic` would flood; defaulting
            # to `editorial` would hide real staleness.  Neither is a finding.
            untypeable.append(
                {"doc": am.doc, "rev": am.attrs.get("rev"), "date": am.attrs.get("date")}
            )
            continue

        edges = PROPAGATION.get(kind, {"*"})
        if not edges:
            non_propagating += 1
            continue
        if not touches:
            continue

        am_ts = _date_ts(am.attrs.get("date"))
        for target_id in touches:
            if target_id not in definitions:
                continue  # section refs and unknown ids carry no citation graph
            for site in sites.get(target_id, []):
                if site.doc == definitions[target_id]:
                    continue  # the defining document is not suspect of itself
                site_ts = touched_ts.get(site.doc, 0)
                if am_ts and site_ts and site_ts >= am_ts:
                    continue  # dependent changed after the amendment; acknowledged
                if (target_id, site.doc) in cleared_keys:
                    continue
                suspects.append(
                    {
                        "object": target_id,
                        "amended_by": f"{am.doc}#{am.attrs.get('rev')}",
                        "change_kind": kind,
                        "suspect_doc": site.doc,
                        "suspect_line": site.line,
                        "amendment_date": am.attrs.get("date"),
                    }
                )

    return _metrics(suspects, untypeable, non_propagating, amendments, cleared)


def _date_ts(date_str) -> int:
    if not date_str:
        return 0
    import datetime

    try:
        return int(
            datetime.datetime.strptime(date_str, "%Y-%m-%d")
            .replace(tzinfo=datetime.timezone.utc)
            .timestamp()
        )
    except ValueError:
        return 0


def _metrics(suspects, untypeable, non_propagating, amendments, cleared) -> dict:
    by_object = collections.Counter(s["object"] for s in suspects)

    # ADDENDUM-C §8.4 — the triple.  A bare "cleared" count is gamed by
    # bulk-clearing, so latency and re-suspect ride alongside, and all three
    # are diagnostics rather than targets.
    latencies = []
    for c in cleared:
        latencies.append(c["ts"])
    re_suspect = sum(1 for c in cleared if (c["id"], c["target"]) in
                     {(s["object"], s["suspect_doc"]) for s in suspects})

    metrics = {
        "open": len(suspects),
        "median_clear_latency_days": (
            round(statistics.median(latencies) / 86400, 1) if len(latencies) > 1 else None
        ),
        "re_suspect_rate": (
            round(re_suspect / len(cleared), 3) if cleared else None
        ),
        "_note": "diagnostics, not targets (SBC-00-ADDENDUM-C §8.4)",
    }
    if not cleared:
        metrics["VACUOUS"] = (
            "no Clears-Suspect trailers exist yet; latency and re-suspect rate "
            "are undefined rather than good"
        )

    coverage = {
        "amendments_total": len(amendments),
        "typed": len(amendments) - len(untypeable),
        "untypeable": len(untypeable),
        "non_propagating": non_propagating,
    }
    if len(amendments):
        pct = 100 * coverage["typed"] / len(amendments)
        coverage["typed_pct"] = round(pct, 1)
        if pct < 5:
            coverage["WARNING"] = (
                f"only {coverage['typed']}/{len(amendments)} amendments declare a "
                "ChangeKind, so suspicion is computable for almost none of the "
                "corpus. A low open count reflects missing metadata, not a "
                "clean corpus (SBC-INV-19)."
            )

    return {
        "metrics": metrics,
        "coverage": coverage,
        "clearings_found": len(cleared),
        "suspect_objects": by_object.most_common(15),
        "suspects": suspects[:200],
        "suspects_truncated": max(0, len(suspects) - 200),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--json", help="write the full report here")
    args = ap.parse_args()

    root = pathlib.Path(args.root).resolve()
    result = compute(root)

    summary = {k: v for k, v in result.items() if k != "suspects"}
    print(json.dumps(summary, indent=2, ensure_ascii=False))

    if args.json:
        pathlib.Path(args.json).write_text(
            json.dumps(result, indent=2, ensure_ascii=False)
        )
        print(f"\n[wrote {args.json}]", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
