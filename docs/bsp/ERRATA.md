<!-- ERRATA.md — Living Creator clock-source issue log activated from pinned SBC template. -->
# ERRATA — CREATOR

**Family:** CREATOR
**Phase docs:** `docs/bsp/CREATOR-*.md`
**Governed by:** [SBC-00](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-CONCEPTS.md) SBC-INV-7 and SBC-INV-10; [Addendum B](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-ADDENDUM-B.md). Activated 2026-10-11 from the pinned parent ERRATA template. No historical issue absence or broad family migration is inferred.

## Status legend

| Icon | Meaning |
| --- | --- |
| 🟢 | resolved — fix landed and verification recorded |
| 🟡 | diagnosed — root cause known and fix prescription clear |
| 🔴 | open — undiagnosed or unresolved |
| ⚪ | deviation-pending-ratification — awaiting §15 amendment |

## Open Questions

| EOQ id | Errata | Ask |
| --- | --- | --- |
| *(none)* | — | — |

The owner accepted source admission before code; no already disposed policy question is fabricated. Any new normative choice stops execution for owner review.

## Index

| Id | Title | Status | Phase | First seen |
| --- | --- | --- | --- | --- |
| `ERRATA-001` | Primary-only parser loses explicit virtual PLL source intent | 🟡 | CREATOR-00 §5 | 2026-10-11 |

## ERRATA-001 — Primary-only parser loses explicit virtual PLL source intent

**Status:** 🟡 diagnosed — owner recommendation accepted; implementation/verification pending
**First seen:** 2026-10-11
**Owning phase:** CREATOR-00, [concepts](CREATOR-00-CONCEPTS.md)

### Symptom

The unchanged H747 IOC declares virtual HSE source, while parser output lacks a PLL source and the generated PAC sequence falls through to HSI. Actual regenerated CM7 compilation separately fails in disabled logging macro syntax; missing nullable HSE renders literal `none` in diagnostics.

### Root cause

`bsp/ioc.rs` reads only `RCC.PLLSource`; the actual IOC has `RCC.PLLSourceVirtual`. The template selects HSI for its nullable string. Existing ownership admission did not resolve alternate key agreement. Logging/null-render syntax is a separate mechanical defect, not source admission authority.

### Fix

The owner accepted CREATOR-00 §5 source admission/agreement and unresolved diagnostics, preserving §6 unknown HSE. Implement in parser/generator/template; preserve unowned paths and existing optional logging boundary. Do not infer a hardware frequency or edit generated Rust directly. No resolving commit exists yet.

### Verification

Meaningful fixtures in already-admitted targets/unit modules, negative controls, genuine rebuilt Creator and unchanged single H747 IOC, retained generation delta and canonical CM7 diagnostic; no broad sweep or hardware claim. Results belong in execution receipts; this entry remains diagnosed until an actual resolving commit is recorded.

### Tracking

CREATOR-00 §15 revision 0.1.0 records accepted semantics before code. Old generation/compiler evidence and the root-owned release wave remain retained separately; no obsolete generation delta is restored.

## How to add an entry

1. Assign the next monotonically increasing `ERRATA-NNN` id.
2. For a deviation, file the owning phase's §15 amendment in the same change and cite the errata in execution; for a stealth revert, record errata before the reverting change.
3. Add Index and stable EOQ rows for open/deviation states.
4. On resolution retain the entry, record resolving commit and actual verification, and update status. Never delete history.
