<!-- CREATOR-00-CONCEPTS.md — Narrow authority for owned H7 PLL source admission. -->
# CREATOR-00-CONCEPTS — Owned H7 PLL Source Admission

**Document ID:** CREATOR-00-CONCEPTS
**Status:** RATIFIED — accepted clock-source scope only
**Revision:** 0.1.0
**Date:** 2026-10-11
**Owner / ratifier:** Ira Abbott
**Canonical path:** `docs/bsp/CREATOR-00-CONCEPTS.md`

## §0 Authority Policy

The current parent [SBC-00 concepts document](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-CONCEPTS.md) and Addenda [B](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-ADDENDUM-B.md), [C](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-ADDENDUM-C.md), and [D](https://github.com/iraabbott/softoboros.com/blob/ddafeb3ffb20ab4ff924b925d88ca47577f2bafb/docs/spec-before-code/docs/SBC-00-ADDENDUM-D.md) at `ddafeb3ffb20ab4ff924b925d88ca47577f2bafb` own the discipline; used without modification. This document owns only the owner-accepted clock-source admission rules below. [STM32.md](STM32.md) remains the informative legacy guide. No other Creator, chip-family, initialization or hardware phase is ratified here. [ERRATA.md](ERRATA.md) is the living family log.

Normative keywords follow SBC's RFC 2119/8174 relationship, used without modification. Sections cited by §12 consumer obligations are normative. The accepted owner instruction of 2026-10-11 precedes implementation; §15 records that scope.

## §1 Purpose

Preserve explicit IOC PLL source intent when emitting an owned H7 PLL initialization sequence, while retaining absent HSE frequency as unknown.

## §2 Problem Statement

The unchanged H747 IOC declares `RCC.PLLSourceVirtual=RCC_PLLSOURCE_HSE` but omits `RCC.PLLSource` and `RCC.HSE_VALUE`. The parser reads only the primary source key. The PAC template's nullable value falls through to HSI. The accepted non-null ownership guard fixed who emits initialization, not which clock intent may be admitted. ERRATA-001 records this diagnosed mismatch. A generated CM7 build also exposed separate logging/null diagnostic syntax defects.

## §3 Canonical Glossary [Normative]

| Term | Relationship |
| --- | --- |
| Clock ownership | As defined in `src/bin/creator/bsp/ir.rs` `Clocks.init_by`; used without modification. Existing `bsp_gen.rs` H7 default ownership and explicit core override remain unchanged. |
| PLL source | As defined in `Clocks.pll_src`; adapted: the explicit IOC key admission/agreement rules in §5 apply before an owned H7 PLL sequence is emitted. |
| HSE frequency | As defined in `Clocks.hse_hz`; used without modification: optional frequency in Hz, not a required board default. |
| Admitted IOC source keys | Owned by CREATOR-00 §5: `RCC.PLLSource` and `RCC.PLLSourceVirtual` interpreted as the same existing source intent. |
| Unknown frequency | Absence in the existing optional `hse_hz`; does not declare a measured or assumed numeric frequency. |

## §4 Source-of-Truth Map [Normative]

| Concept | Authoritative owner | Implementation / evidence |
| --- | --- | --- |
| Source admission and disagreement | This document §5/§9 | `ioc.rs`, `bsp_gen.rs`; existing admitted test/unit modules |
| Optional HSE representation | Existing IR `Clocks.hse_hz`; §6 preserves it | `bsp/ir.rs`, PAC diagnostics |
| Existing core selection / emission guards | Existing `Clocks.init_by`, `bsp_gen.rs`, PAC ownership guards | `tests/bsp_clock_ownership.rs` |
| IOC authored keys and tokens | Input IOC | Read-only input bytes; no generator rewrite |
| Family clock register encoding | Existing H7 template / PAC authority | Not amended or qualified here |
| SBC status, errata and amendment grammar | Pinned parent SBC and Addenda B/C/D | Local operational mappings and generated index |

## §5 Source Admission [Normative]

**INV-CREATOR-1.** Before emitting an owned H7 PLL initialization sequence, the generator MUST admit explicit `RCC.PLLSource` or, when the primary key is absent, `RCC.PLLSourceVirtual`. It MUST normalize only existing `HSI`, `CSI`, `HSE` and their `RCC_PLLSOURCE_*` equivalents. If both keys exist, their normalized sources MUST agree. Conflicting, unsupported or unresolved source intent MUST produce a named generation diagnostic rather than implicit HSI selection.

The validation applies to emitted owned H7 PLL initialization. It MUST NOT impose a new initialization obligation on external/unowned paths or non-PLL system-clock paths. Explicit core/owner overrides retain their existing meaning.

## §6 Optional HSE Frequency [Normative]

**INV-CREATOR-2.** Missing HSE frequency MUST remain unknown in the existing optional IR field and diagnostic reporting. The generator MUST NOT infer a board frequency, derive it from PLL values, activate the reserved `STM32_RCC_HSE_HZ` override, or represent an absent value as measured zero.

The current diagnostic-only frequency field does not add a new frequency prerequisite or hardware qualification claim. Valid numeric IOC HSE values retain their existing representation.

## §7 Registration Policy

The source set is the existing HSI/CSI/HSE set, not a new clock encoding. Changes to this document's admission, disagreement or absence rules require an owner-ratified §15 amendment (Standards Action as defined by SBC). This document adds no source value or hardware default.

## §8 Traceability

Implementation is MANUAL against this accepted contract; generated BSP output is DERIVED through the genuine Creator pipeline. These traceability terms are as defined by SBC §7, used without modification. Existing production code is not retroactively declared VALIDATED.

## §9 Preservation [Normative]

**INV-CREATOR-3.** Implementation MUST preserve non-null ownership guards, external/unowned initialization, existing power defaults, package versions, locks and finite staging guards. Mechanical logging/null diagnostic syntax repairs MUST remain separate from source admission and MUST NOT introduce a frequency assumption or new sink behavior.

## §10 Reconciliation [Normative]

| Boundary | Relationship / upstream authority | Local representation and mutation rights | Divergence policy | Downstream consumers | Conformance test owner |
| --- | --- | --- | --- | --- | --- |
| CubeMX IOC source intent | derive; authored IOC keys/tokens | Existing `Clocks.pll_src`; normalize only §5 tokens, never mutate IOC | Reject disagreement/unsupported/unresolved owned PLL source | Generated owned H7 PAC initialization | CREATOR owner; existing admitted tests/unit modules |
| Existing optional IR frequency | compose; `Clocks.hse_hz` | Preserve Option; no new default or override | Unknown remains unknown | Generated diagnostics | CREATOR owner; existing admitted template tests |
| Existing H7 ownership | compose; IR and generator selectors | No selector/default changes | Retain external/unowned paths | Split CM7/CM4 outputs | Existing clock-ownership regression target |
| SBC discipline | compose; pinned SBC authority | This instance and operational mappings only | Parent authority wins; do not rewrite historical guides | Reviewer/spec index | Local spec-index tests |

## §11 Non-Goals

No full Creator-family ratification, fractional PLL/range work, RCC sequence redesign, oscillator readiness claim, new family admission, HSE override implementation, disabled-logging evaluation policy, 252-IOC sweep, generated Rust hand edit, hardware action or release/publication approval is included.

## §12 Acceptance

### Document acceptance

- [x] Owner accepted the narrow source-key/agreement/absence rules before code on 2026-10-11.
- [x] §0, §3/§4, §10 and §15 assign authority, preserve adjacent behavior and delimit scope.
- [x] Optional HSE remains unknown; no wider family/physical conformance is ratified.
- [x] Family errata is activated from the pinned SBC template; the diagnosed failure is retained.

### Consumer obligations — open at ratification

- [ ] Existing admitted tests cover §5/INV-CREATOR-1 primary/virtual/equal/conflicting/unsupported/absent cases and external/non-owner paths, with meaningful negative controls.
- [ ] Existing tests cover §6/INV-CREATOR-2 unknown and numeric diagnostic rendering.
- [ ] Source/locks/ownership preservation proves §9/INV-CREATOR-3.
- [ ] Rebuilt Creator, single unchanged H747 IOC and exact CM7 profile retain actual results and generation delta; compilation is separate from hardware readiness.

These are implementation obligations, not prerequisites for recording the owner-accepted specification. No test/build/phase success is fabricated.

## §13 Files Cited

`src/bin/creator/bsp/ioc.rs`, `src/bin/creator/bsp/ir.rs`, `src/bin/creator/bsp_gen.rs`, `src/bin/creator/bsp/templates/pac.rs.jinja`, `tests/bsp_clock_ownership.rs`, `examples/stm32h747i-disco/DiscoBiscuit/DiscoBiscuit.ioc`, [STM32.md](STM32.md), [ERRATA.md](ERRATA.md), pinned parent SBC/Addenda.

## §14 Unblocks

Only the accepted source admission implementation and bounded single-input tests/render/build diagnostics. Other BSP, release, family and physical gates remain independently open.

## §15 Change Log

### 0.1.0 — 2026-10-11 — ratified

**Author:** Ira Abbott (ratifier); OpenAI Codex (recording)
**Change kind:** semantic
**Touches:** INV-CREATOR-1, INV-CREATOR-2, INV-CREATOR-3
**Summary:** Records owner-accepted explicit source-key admission, normalized agreement and unresolved-source diagnostics for emitted owned H7 PLL initialization; preserves unknown optional HSE.

#### Rationale

The actual IOC carries HSE intent in the virtual source key, while the primary-only parser yields a nullable value and generated HSI fallback. Silent priority on conflicting keys, accepting arbitrary tokens, and importing the board's 25 MHz runtime assumption were considered and rejected: they would hide disagreement or fabricate input intent. The existing accepted clock-owner correction remains unchanged. The scope ratifies only this narrow contract, not legacy Creator/BSP narratives, register encodings, reserved frequency overrides, other phases or hardware. ERRATA-001 remains diagnosed until a resolving commit and actual verification are recorded.
