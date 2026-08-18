<!--
SBC-COMPATIBILITY.md - Current-SBC adoption boundary for the rlvgl documentation corpus.
-->

# rlvgl Current-SBC Compatibility Map

**Status:** Owner-directed adoption baseline — 2026-08-14

**Current authority:** The Softoboros
[SBC-00 concepts document](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-CONCEPTS.md)
and Addenda B, C, and D at the same revision.

**Scope:** The multi-phase rlvgl documentation corpus under `docs/` and
`chipdb/`. This is an operational compatibility map, not a second definition
of SBC terms, object kinds, or ratification rules.

## Adoption Boundary

The owner directed rlvgl to reconcile its documentation with the current SBC
on 2026-08-14. The current SBC is authoritative for every new phase document,
every newly ratified phase, and every material amendment made after this
adoption record.

Documents and implementation evidence that predate this record remain
historical evidence for the revisions they describe. They are not silently
rewritten, re-ratified, or treated as current-format documents by this map. A
future behavior change MUST use a current-SBC-ratified phase document; a
legacy document cannot be used as a shortcut around that gate.

## Local Integration Points

| SBC responsibility | rlvgl artifact |
|---|---|
| Agent-facing authority pointer | `CLAUDE.md` “Current SBC authority and rlvgl adoption” |
| Family prefix allocation | [PREFIX-REGISTRY.md](PREFIX-REGISTRY.md) |
| Deterministic local projection | [README.md](README.md), `make spec-index`, and `make spec-index-check` |
| CPY family errata | [`docs/cpython/ERRATA.md`](../cpython/ERRATA.md) |
| MPY family errata | [`docs/concepts/ERRATA.md`](../concepts/ERRATA.md) |
| WLD family errata | [`docs/wayland/ERRATA.md`](../wayland/ERRATA.md) |
| Existing family errata | `docs/beetle-esp32p4/ERRATA.md` and `docs/disco-test-and-debug/ERRATA.md` |

Before a family without a current errata log receives its next behavior change
or semantic/scope/retirement amendment, its owner MUST create the family log
from the SBC `ERRATA.md` template. That migration is deliberately explicit;
an empty or absent historical log is not evidence that no prior issue existed.

## Family Status Map

| Family | Canonical entry point | Current reconciliation state |
|---|---|---|
| APP | `docs/app-schema/00-concepts.md` | Current-shape phase documents; focused `APP-05-A` and `APP-06-A` remain historical analyses. |
| AUDIO / AUDIO-METERS | `docs/audio/01-codec-bringup.md`; `docs/audio-meters/00-concepts.md` | Current-shape active docs; create family errata before the next material amendment. |
| BBB | `docs/beaglebone-black/README.md` and `05-zephyr-prong.md` | Current-shape phase material; the bare-metal bring-up guide is an as-built narrative, not a phase gate. |
| BEETLE | `docs/beetle-esp32p4/BEETLE-00-CONCEPTS.md` | Current-shape and existing family errata log. |
| BEETLE-IDF | `docs/beetle-esp32p4-idf/BEETLE-IDF-00-CONCEPTS.md` | Current-shape phase material; create family errata before the next material amendment. |
| CPY | `docs/cpython/CPY-00-CONCEPTS.md` | Current-shape family with local errata. CPY-00 is ratified; CPY leads CPython/PyO3 and crate unification/partition planning. CPY-01 through CPY-09 remain separately gated Draft phases. |
| CRATES-CI | `docs/crates-ci/CRATES-CI-00-CONCEPTS.md` | Current-shape family; create family errata before the next material amendment. |
| DCB | `docs/concepts/DCB-00-CONCEPTS.md` | Canonical root remains authoritative; `DCB-*-A` files are resolved historical analyses and MUST NOT authorize behavior. |
| DPR | `docs/concepts/DPR-00-CONCEPTS.md` | Draft; no behavior implementation is authorized by this adoption map. |
| FONT / INPUT / REND / WID | Their respective `docs/concepts/*-00-CONCEPTS.md` roots | Current-shape family documents; create a family errata log before the next material amendment. |
| KI2C | `docs/concepts/KI2C-00-CONCEPTS.md` | KI2C-07 is hardware-blocked. Its physical conformance work remains evidence-gated and must receive a current §15 bridge before resuming behavior work. |
| LPAR | `docs/concepts/LPAR-00-CONCEPTS.md` | Later phases are current-shape; the three legacy change-log bridges below remain explicit migration work. |
| MPY | `docs/concepts/MPY-00-CONCEPTS.md` | MPY-01 and MPY-02 are ratified with their phase PCDNs resolved; golden protocol vectors remain required before MPY-03, and MPY-03 through MPY-09 remain separately gated Draft phases. |
| QT | `docs/qt-support/00-concepts.md` | Current-shape family; create family errata before the next material amendment. |
| RATATUI / SCTD | Their respective `docs/concepts/*-00-CONCEPTS.md` roots | Current-shape family documents; create a family errata log before the next material amendment. |
| WLD | `docs/wayland/WLD-00-CONCEPTS.md` | Current-shape Draft family with local errata; PCDN-WLD-001 and PCDN-WLD-002 are resolved, three PCDNs remain open, and no implementation is authorized. |
| CHIPS-* / DISCO-* | Their canonical `chipdb/` and `docs/disco-*/` roots | Existing local documentation remains a legacy baseline until each family activates its current-SBC errata and amendment workflow. |

## Explicit Legacy Bridges

The following documents already carry their domain decisions and historical
evidence, but their change-log section has a pre-current-SBC number. Their
content is preserved. Before the next material amendment, append a current
`## 15 Change Log` block-form entry and use the current `ChangeKind` and
rationale requirements; do not rewrite earlier entries or infer missing
evidence.

| Document | Existing log section | Bridge classification |
|---|---|---|
| `docs/concepts/LPAR-01-BASELINE.md` | §13 | Legacy structure; no current behavior change is opened here. |
| `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md` | §14 | Legacy structure; no current behavior change is opened here. |
| `docs/concepts/LPAR-10-LAYOUT.md` | §14 | Legacy structure; no current behavior change is opened here. |
| `docs/concepts/KI2C-07-HARDWARE-CONFORMANCE.md` | §12 | Hardware-blocked; bridge before physical conformance resumes. |

The following documents are deliberately excluded from the phase-gate
inventory: `docs/releases/roadmap-pre-v0.2.md`, the BeagleBone bare-metal
bring-up narrative, and resolved DCB/APP sub-letter analyses. They are
backlog, as-built, or historical-decision artifacts. They MUST NOT authorize a
new behavior change.

## Current Open Gates

- **CPY:** CPY-00 is ratified with `PCDN-CPY-00-001` through
  `PCDN-CPY-00-003` accepted as amended. CPY-01 through CPY-09 remain Draft;
  CPY-01's six baseline selections and CPY-02's six topology decisions are
  resolved, CPY-03 has resolved four runtime-policy decisions, and CPY-04 has
  resolved all six director-binding policy decisions. The exact
  manifest/rootfs/board artifacts, an actual MPY Handoff Record, measured
  queue/performance budgets, binding implementation/conformance evidence, and
  per-phase ratification remain open. CPY planning leads the future shared
  crate topology, but CPY-00 authorizes no code or changes the active MPY/WLD
  authorities.
- **MPY:** Later phase ratification remains blocked by their own PCDNs and,
  where specified, compile, exception, cache/shared-memory, measured-budget,
  or board evidence.
- **WLD:** WLD-00 is Ratified with all five PCDNs resolved. WLD-01 remains
  Draft pending phase ratification, and WLD-02 remains Draft and blocked by
  WLD-01. Release parity still requires the complete compositor, isolation,
  resource, documentation, versioning, and changelog evidence.
- **KI2C-07:** Physical-board facts and read-only probe evidence are missing;
  hardware-blocked is not a conformance result.
- **Legacy bridges:** The four documents above require their §15 bridge before
  their next material amendment. This is a documentation migration, not a
  license to change their frozen behavior.

## Maintenance Rule

Update this map and [PREFIX-REGISTRY.md](PREFIX-REGISTRY.md) in the same change
whenever a family activates its errata log, completes a legacy bridge, or adds
a new family prefix. Regenerate the local documentation index with
`make spec-index` and verify it with `make spec-test spec-index-check`.
