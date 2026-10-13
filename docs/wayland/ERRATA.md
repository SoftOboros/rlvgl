<!--
ERRATA.md - Living errata log for the WLD family.
-->

# ERRATA — WLD

**Family:** WLD
**Phase docs:** `docs/wayland/WLD-*.md`
**Governed by:** the current parent
[SBC-00 concepts document](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-CONCEPTS.md)
and Addenda B, C, and D, as adopted in
[`SBC-COMPATIBILITY.md`](../spec-index/SBC-COMPATIBILITY.md).

## Status legend

| Icon | Meaning |
|---|---|
| 🟢 | resolved — fix landed and verification evidence recorded |
| 🟡 | diagnosed — root cause known and fix prescription clear |
| 🔴 | open — undiagnosed or unresolved |
| ⚪ | deviation-pending-ratification — awaiting a §15 amendment |

## Open questions

| EOQ id | Errata | Ask |
|---|---|---|
| *(none)* | — | — |

## Index

| Id | Title | Status | Phase | First seen |
|---|---|---|---|---|
| *(none)* | — | — | — | — |

## How to add an entry

1. Assign the next monotonically increasing `ERRATA-NNN` identifier.
2. For a deviation from ratified specification, add a ⚪ entry and the
   affected phase's §15 amendment in the same documentation change.
3. For a stealth revert, land the errata entry before the reverting change.
4. Add an Index row and, for ⚪ or 🔴, an `EOQ-NNN-ERRATA-NNN` row above.
5. Keep entries permanently. On resolution, record the resolving commit and
   verification evidence, change the status to 🟢, and remove its Open
   Questions row.
