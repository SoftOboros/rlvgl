<!--
ERRATA.md - Living errata log for the flat MPY concepts family.
-->

# ERRATA — MPY

**Family:** MPY
**Phase docs:** `docs/concepts/MPY-*.md`
**Governed by:** the current parent
[SBC-00 concepts document](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-CONCEPTS.md)
(SBC-INV-7 and SBC-INV-10) and Addendum B, as adopted in
[`CLAUDE.md`](../../CLAUDE.md).

MPY uses the existing flat `docs/concepts/` lane, so this file is the
family-local errata log alongside its phase documents.

## Status legend

| Icon | Meaning |
|---|---|
| 🟢 | resolved — fix landed and verification evidence recorded |
| 🟡 | diagnosed — root cause known and fix prescription clear |
| 🔴 | open — undiagnosed or unresolved |
| ⚪ | deviation-pending-ratification — awaiting a §15 amendment |

---

## Open Questions

*One row per ⚪ or 🔴 entry; remove it when the entry reaches 🟢 or 🟡.*

| EOQ id | Errata | Ask |
|---|---|---|
| *(none)* | — | — |

---

## Index

| Id | Title | Status | Phase | First seen |
|---|---|---|---|---|
| ERRATA-001 | Infallible text readback aborts snapshot preparation | 🟡 diagnosed | MPY-04 §8.2; MPY-05 §6 | 2026-10-10 |

---

## How to add an entry

1. Assign the next monotonically increasing `ERRATA-NNN` id.
2. For a deviation from ratified specification, file a ⚪ entry and the
   affected phase's §15 amendment in the same change; cite the id in the
   execution commit subject.
3. For a stealth revert, the errata entry MUST land before the reverting
   change, as required by SBC-INV-7.
4. Add an Index row and, for ⚪ or 🔴, an `EOQ-NNN-ERRATA-NNN` row above.
5. Keep entries permanently. On resolution, record the resolving commit and
   verification evidence, change the status to 🟢, and remove its Open
   Questions row.

---

## ERRATA-001 — Infallible text readback aborts snapshot preparation

**Status:** 🟡 diagnosed

**First seen:** 2026-10-10

**Phase:** MPY-04 §8.2 complete-page allocation guarantee; MPY-05 §6 native projection

**Classification:** implementation defect; no requirement deviation or new PCDN

**Evidence:** The nonempty Button snapshot allocation-failure sweep aborted on
an 11-byte allocation (`SIGABRT`) in `MpyActor::property`. Both Button and Label
used `String::from(self.text())` for property readback, which cannot return the
existing `RegistryError` on allocation failure. The original failure remains
preserved in the external recovery evidence at
`recovery/mpy/implementation-snapshot.log`.

**Prescription:** In those two property-readback arms only, reserve an empty
String with `try_reserve_exact`, map allocation failure to the existing
`RegistryError::Capacity { kind: SnapshotValues }`, then copy with `push_str`.
Keep actor direction preparation, constructors and invocation paths unchanged.
Retry the original nonempty complete-page sweep and independently inject Label
readback failure; demonstrate that the cursor position/sequence/end ownership
and Stage/subscription revisions do not change on returned preparation failure.
If another external infallible allocation appears, stop the affected proof and
record its exact boundary instead of extending the correction recursively.

**Resolution:** Candidate correction and verification pending. No resolving
commit is recorded; this entry remains diagnosed until the fix lands with its
verification evidence.
