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
| CREATOR family errata | [`docs/bsp/ERRATA.md`](../bsp/ERRATA.md) |
| CPY family errata | [`docs/cpython/ERRATA.md`](../cpython/ERRATA.md) |
| MPY family errata | [`docs/concepts/ERRATA.md`](../concepts/ERRATA.md) |
| KI2C family errata | [`docs/concepts/KI2C-ERRATA.md`](../concepts/KI2C-ERRATA.md) |
| WLD family errata | [`docs/wayland/ERRATA.md`](../wayland/ERRATA.md) |
| CRATES-CI family errata | [`docs/crates-ci/ERRATA.md`](../crates-ci/ERRATA.md) |
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
| CREATOR | `docs/bsp/CREATOR-00-CONCEPTS.md` | Current-SBC errata/concepts activated 2026-10-11 for accepted clock-source admission only; all other Creator/BSP narratives remain legacy and no broader conformance is ratified. |
| CPY | `docs/cpython/CPY-00-CONCEPTS.md` | Current-shape family with local errata. CPY-00 through CPY-02 are ratified; CPY leads CPython/PyO3 and crate unification/partition planning. CPY-03 through CPY-09 remain separately gated Draft phases. |
| CRATES-CI | `docs/crates-ci/CRATES-CI-00-CONCEPTS.md` | Current-shape family with local errata activated 2026-10-05. CRATES-CI-01b is a drafted mechanism correction; its defect/decision and proof remain open. |
| DCB | `docs/concepts/DCB-00-CONCEPTS.md` | Canonical root remains authoritative; `DCB-*-A` files are resolved historical analyses and MUST NOT authorize behavior. |
| DPR | `docs/concepts/DPR-00-CONCEPTS.md` | Draft; no behavior implementation is authorized by this adoption map. |
| FONT / INPUT / REND / WID | Their respective `docs/concepts/*-00-CONCEPTS.md` roots | Current-shape family documents; create a family errata log before the next material amendment. |
| KI2C | `docs/concepts/KI2C-00-CONCEPTS.md` | KI2C family errata activated 2026-10-06. KI2C-07 received its current §15 scope bridge for owner-ratified identity-only RLVGL smoke with existing reporting. Physical conformance remains hardware-blocked; temperature-dependent CCPS work has separate validation gates. |
| LPAR | `docs/concepts/LPAR-00-CONCEPTS.md` | Later phases are current-shape; the three legacy change-log bridges below remain explicit migration work. |
| MPY | `docs/concepts/MPY-00-CONCEPTS.md` | MPY-01 through MPY-05 are ratified. Native substrate evidence is separate from complete wire/interpreter/board conformance; MPY-04-017 remains proposal-only. Owner decisions 2026-10-06 accept neutral subscription metadata, atomic full-page preparation and owning-Stage revision advancement for actual subscription changes; implementation and fresh handoff remain open. MPY-06/07 are owner-accepted but unratified; MPY-08/09 remain Draft. |
| QT | `docs/qt-support/00-concepts.md` | Current-shape family; create family errata before the next material amendment. |
| RATATUI / SCTD | Their respective `docs/concepts/*-00-CONCEPTS.md` roots | Current-shape family documents; create a family errata log before the next material amendment. |
| WLD | `docs/wayland/WLD-00-CONCEPTS.md` | Current-shape family with local errata; WLD-00/01 are ratified and all five root PCDNs are resolved. WLD-01 acceptance evidence remains incomplete; WLD-02 stays Draft pending its prerequisites and ratification. |
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
| `docs/concepts/KI2C-07-HARDWARE-CONFORMANCE.md` | §12 | Current §15 bridge completed 2026-10-06; historical §12 preserved. Physical conformance remains evidence-gated. |

The following documents are deliberately excluded from the phase-gate
inventory: `docs/releases/roadmap-pre-v0.2.md`, the BeagleBone bare-metal
bring-up narrative, and resolved DCB/APP sub-letter analyses. They are
backlog, as-built, or historical-decision artifacts. They MUST NOT authorize a
new behavior change.

## Current Open Gates

- **CPY:** CPY-00 is ratified with `PCDN-CPY-00-001` through
  `PCDN-CPY-00-003` accepted as amended. CPY-01 and CPY-02 are also ratified;
  CPY-03 through CPY-09 remain Draft. CPY-03 has resolved four runtime-policy
  decisions and now has a bounded native service, Unix readiness, and retained
  v1/v2 host capacity matrices, while semantic-workload and BBB capacity
  evidence remain open. CPY-04 has
  resolved all six director-binding policy decisions, and CPY-05 has resolved
  five frame-policy decisions while retaining measured slot capacities.
  CPY-06 has resolved all six embedded-Linux policy decisions, and CPY-07 has
  resolved three host policies while retaining two measured/topology gates.
  CPY-08 has resolved all six packaging-policy decisions, and CPY-09 has
  resolved three closure policies while retaining three evidence/release
  decisions. The exact
  remaining target/rootfs/board execution artifacts, measured queue/performance
  budgets, binding implementation/conformance evidence, and
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
- **KI2C-07:** Physical-board facts and read-only identity/health probe evidence
  are missing; hardware-blocked is not a conformance result. Temperature validation
  blocks only the specific CCPS dependencies requiring it, not generic RLVGL release.
- **Legacy bridges:** The three LPAR documents above still require their §15 bridge before
  their next material amendment; KI2C-07 completed its bridge on 2026-10-06. This is a documentation migration, not a
  license to change their frozen behavior.

## Maintenance Rule

Update this map and [PREFIX-REGISTRY.md](PREFIX-REGISTRY.md) in the same change
whenever a family activates its errata log, completes a legacy bridge, or adds
a new family prefix. Regenerate the local documentation index with
`make spec-index` and verify it with `make spec-test spec-index-check`.
