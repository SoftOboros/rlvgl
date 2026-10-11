# ERRATA — KI2C

**Family:** KI2C
**Phase docs:** `docs/concepts/KI2C-*.md`
**Governed by:** the current parent
[SBC-00 concepts document](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-CONCEPTS.md)
(SBC-INV-7 and SBC-INV-10) and Addendum B, as adopted in
[CLAUDE.md](../../CLAUDE.md) and the
[compatibility map](../spec-index/SBC-COMPATIBILITY.md).

Activated October 6, 2026 from the pinned SBC errata template. This dedicated
log governs the flat KI2C lane; `docs/concepts/ERRATA.md` remains MPY-only.
Activation records no inferred historical issues beyond the diagnosed entry below.

## Status legend

| Icon | Meaning |
| --- | --- |
| 🟢 | resolved — fix landed and verification evidence recorded |
| 🟡 | diagnosed — root cause known and fix prescription clear |
| 🔴 | open — undiagnosed or unresolved |
| ⚪ | deviation-pending-ratification — awaiting a §15 amendment |

---

## Open Questions

| EOQ id | Errata | Ask |
| --- | --- | --- |
| *(none)* | — | — |

The owner disposed of the scope question on October 6. Missing physical
evidence remains a qualification gate, not an unresolved policy question.

---

## Index

| Id | Title | Status | Phase | First seen |
| --- | --- | --- | --- | --- |
| `ERRATA-001` | Read-only smoke requires a temperature read that needs configuration | 🟡 | KI2C-07 §6 | 2026-10-05 |

---

## ERRATA-001 — Read-only smoke requires temperature configuration

**Status:** 🟡 diagnosed — owner scope disposition ratified; documentation amendment recorded, physical evidence remains absent
**First seen:** 2026-10-05
**Owning phase:** KI2C-07, [hardware conformance handoff](KI2C-07-HARDWARE-CONFORMANCE.md)

### Symptom

Before the October 6 scope amendment, KI2C-07 §6 required one coherent STTS22H
temperature read without changing configuration. A fresh accepted driver
returns `NotConfigured` before I/O until configuration succeeds.

### Root cause

[KI2C-02 §6/§7](KI2C-02-STTS22H.md) requires successful configuration with BDU
and autoincrement before coherent temperature capture. The original physical
smoke requirement combined that measurement with strictly read-only operation
and offered no accepted observed-configuration adoption API.

### Fix

Owner decisions 4 and 6, accepted October 6, select strict read-only identity
and existing reporting for RLVGL; PTN3460 and PCM3168A retain typed health
semantics rather than silicon-identity claims. Temperature validation remains
required for the specific CCPS functions that depend on it, and prototype
wiring revision differences remain CCPS rig qualification concerns. Neither
is a generic RLVGL publication prerequisite. No driver API, configuration
write, reporting schema or pin map is changed.

### Verification

Check KI2C-07 §6's operation scope, unchanged §4/§5 safety prerequisites and
§7 record, explicit §8 release/physical distinction, and preserved legacy §12.
The [owner decision receipt](../releases/v0.2.7-embedded-owner-decision.md)
records these documentary checks. No board operation or temperature/wiring
qualification is inferred. Record the actual resolving commit before claiming
a committed resolution; this entry remains diagnosed in the candidate.

### Tracking

KI2C-07 §15 revision 0.1.0 ratifies the scope amendment before changed
requirements. The historical wave2 conflict/options and August 29 wiring
observations remain preserved in
[the amended embedded receipt](../releases/v0.2.7-embedded-next-slice.md).
No EOQ is fabricated for a policy already disposed of by the owner.

---

## How to add an entry

1. Assign the next monotonically increasing `ERRATA-NNN` id.
2. For a deviation from ratified specification, file the affected phase's §15
   amendment in the same change and cite the errata id in the execution commit.
3. For a stealth revert, land the errata before the reverting change.
4. Add an Index row and, for ⚪ or 🔴, a stable `EOQ-NNN-ERRATA-NNN` row.
5. Retain entries permanently; on resolution record the resolving commit and
   verification evidence, update status and remove its Open Questions row.
