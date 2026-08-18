<!--
PREFIX-REGISTRY.md - Named allocation authority for rlvgl current-SBC families.
-->

# rlvgl SBC Prefix Registry

**Registry owner:** Ira Abbott

**Authority:** The Softoboros
[SBC-00 concepts document](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-CONCEPTS.md)
§8 and Addendum C §4.3. This registry is the local allocation record; it does
not redefine identifier grammar or canonicalization.

## Current Family Prefixes

| Prefix | Named family owner | Canonical entry point | Documentation locus |
|---|---|---|---|
| APP | Ira Abbott | `docs/app-schema/00-concepts.md` | `docs/app-schema/` |
| AUDIO | Ira Abbott | `docs/audio/01-codec-bringup.md` | `docs/audio/` |
| AUDIO-METERS | Ira Abbott | `docs/audio-meters/00-concepts.md` | `docs/audio-meters/` |
| BBB | Ira Abbott | `docs/beaglebone-black/README.md` | `docs/beaglebone-black/` |
| BEETLE | Ira Abbott | `docs/beetle-esp32p4/BEETLE-00-CONCEPTS.md` | `docs/beetle-esp32p4/` |
| BEETLE-IDF | Ira Abbott | `docs/beetle-esp32p4-idf/BEETLE-IDF-00-CONCEPTS.md` | `docs/beetle-esp32p4-idf/` |
| CHIPS-ESP | Ira Abbott | `chipdb/rlvgl-chips-esp/` | `chipdb/rlvgl-chips-esp/` |
| CHIPS-MICROCHIP | Ira Abbott | `chipdb/rlvgl-chips-microchip/` | `chipdb/rlvgl-chips-microchip/` |
| CHIPS-SILABS | Ira Abbott | `chipdb/rlvgl-chips-silabs/` | `chipdb/rlvgl-chips-silabs/` |
| CHIPS-TI | Ira Abbott | `chipdb/rlvgl-chips-ti/` | `chipdb/rlvgl-chips-ti/` |
| CRATES-CI | Ira Abbott | `docs/crates-ci/CRATES-CI-00-CONCEPTS.md` | `docs/crates-ci/` |
| DCB | Ira Abbott | `docs/concepts/DCB-00-CONCEPTS.md` | `docs/concepts/` |
| DISCO | Ira Abbott | `docs/disco-platform-guide/README.md` | `docs/disco-*/` |
| DPR | Ira Abbott | `docs/concepts/DPR-00-CONCEPTS.md` | `docs/concepts/` |
| FONT | Ira Abbott | `docs/concepts/FONT-00-CONCEPTS.md` | `docs/concepts/` |
| INPUT | Ira Abbott | `docs/concepts/INPUT-00-CONCEPTS.md` | `docs/concepts/` |
| KI2C | Ira Abbott | `docs/concepts/KI2C-00-CONCEPTS.md` | `docs/concepts/` |
| LPAR | Ira Abbott | `docs/concepts/LPAR-00-CONCEPTS.md` | `docs/concepts/` |
| MPY | Ira Abbott | `docs/concepts/MPY-00-CONCEPTS.md` | `docs/concepts/` |
| QT | Ira Abbott | `docs/qt-support/00-concepts.md` | `docs/qt-support/` |
| RATATUI | Ira Abbott | `docs/concepts/RATATUI-00-CONCEPTS.md` | `docs/concepts/` |
| REND | Ira Abbott | `docs/concepts/REND-00-CONCEPTS.md` | `docs/concepts/` |
| SCTD | Ira Abbott | `docs/concepts/SCTD-00-CONCEPTS.md` | `docs/concepts/` |
| WID | Ira Abbott | `docs/concepts/WID-00-CONCEPTS.md` | `docs/concepts/` |
| WLD | Ira Abbott | `docs/wayland/WLD-00-CONCEPTS.md` | `docs/wayland/` |

## Historical Identifier Aliases

These retained identifiers resolve to the named family above; they are not
available for a new family or a new meaning.

| Historical form | Canonical family |
|---|---|
| `INV-A*`, `INV-W*` | AUDIO-METERS |
| `INV-C*` | CRATES-CI |
| `INV-D*` | DCB |
| `INV-MC*` | CHIPS-MICROCHIP |
| `INV-SL*` | CHIPS-SILABS |

## Allocation Rule

A new family prefix requires an owner-directed update to this file before the
first invariant, PCDN, errata, or phase-code use. Reassigning or retiring a
registered prefix follows the current SBC's Standards Action rule.
