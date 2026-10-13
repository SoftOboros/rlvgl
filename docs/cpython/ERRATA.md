<!--
ERRATA.md - Living errata log for the CPY initiative family.
-->

# ERRATA — CPY

**Family:** CPY

**Phase docs:** `docs/cpython/CPY-*.md`

**Governed by:** the current parent
[SBC-00 concepts document](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-CONCEPTS.md)
(`SBC-INV-7`, `SBC-INV-10`, and `SBC-INV-13`) and Addendum B, as
adopted in [`CLAUDE.md`](../../CLAUDE.md).

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
| *(none)* | — | — | — | — |

---

## How to add an entry

1. Assign the next monotonically increasing `ERRATA-NNN` id.
2. For a deviation from ratified specification, file a ⚪ entry and the
   affected phase's §15 amendment in the same change; cite the id in the
   execution commit subject.
3. For a stealth revert, the errata entry MUST land before the reverting
   change, as required by `SBC-INV-7`.
4. Add an Index row and, for ⚪ or 🔴, an `EOQ-NNN-ERRATA-NNN` row above.
5. Keep entries permanently. On resolution, record the resolving commit and
   verification evidence, change the status to 🟢, and remove its Open
   Questions row.
