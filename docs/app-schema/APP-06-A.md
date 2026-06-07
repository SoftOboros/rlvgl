# APP-06-A — figma/uml layout authoring close-with-deferral

**Status:** **Resolved 2026-05-04 — close-with-deferral.** Both
`figma_export_v1` and `uml_widget_v1` (chapter 01 §5.5 layout
format enum) reclassified as **Coupled deferred** work per
[`CLAUDE.md`](../../CLAUDE.md) §"Initiative retrospective" §5.
`rust_inline_v1` is formally accepted as the primary v0.5 layout
authoring path with no v1 sunset gated until either deferred
format ships against a concrete external consumer.

This file is preserved as the canonical deferral record;
behaviour PRs reference chapter 01 §5.5 (frozen grammar) and the
coupling criteria in §4 below.

## §1 Purpose

Close the only remaining v0.5 open work item on the rlvgl
Application Schema initiative — figma/uml layout authoring —
without shipping either format. Replace the README's open-work
narrative with a structured deferral that names what would
unfreeze either format.

The chapter 01 §5.5 grammar already permits all three enum
values; §6.10 of chapter 03 already articulated the deferred
state ("Don't remove the backdoor before there's a real front
door"). This sub-letter cements that informal disposition into
an explicit close-with-deferral with named gating criteria, so
future agents do not re-derive the question.

## §2 Problem statement

### 2a. What ships today

Layout authoring at v0.5 is `rust_inline_v1` only. The
layout-translator (chapter 02 §7.7) implements `rust_inline_v1`
as a verbatim file copy — `screens[].layout` points at a `.rs`
fragment which is included into the generated crate as
`src/screens/<id>.rs`. Five of six committed round-trip
manifests use `rust_inline_v1` (the sixth is the `bsp_pac`
headless intent which uses it for an LED-blink stub).

The orchestrator hard-errors on `figma_export_v1` and
`uml_widget_v1` at `src/bin/creator/app.rs:2178` with a stale
"land in APP-02c+" message that this PR refreshes to point at
APP-06-A.

### 2b. Why neither format has a v0.5 implementation

**figma_export_v1.** No canonical input format exists. Figma
exports designs through several paths (Figma REST API JSON,
plugin-driven JSON, Figma-to-code plugins like Anima or Locofy
that emit framework-specific code). Picking one without a real
external consumer would lock the project into a specific
toolchain whose JSON shape is not stable across Figma version
updates. The chapter 06 chakra/svelte token reconciliation
(`docs/audio-meters/` precedent) shows what the design-token
half of this looks like; the layout half remains unanchored.

**uml_widget_v1.** No canonical UML widget dialect exists.
Candidate dialects include PlantUML "salt" wireframes, Mermaid's
flowchart-with-icons subset, structurizr's component diagrams,
and custom subsets of UML class diagrams used as widget-tree
declarations. Each dialect carries a different semantics
(salt's grid-based wireframe vs. structurizr's containment
hierarchy vs. PlantUML class diagrams as type-safe widget
declarations). Picking one by fiat — without 3+ candidate
manifests independently arriving at a similar shape — would
likely produce a DSL that doesn't compress real authoring
effort.

### 2c. Why this isn't urgent

Round-trip evidence (chapter 03 §6.10, six committed manifests):
`rust_inline_v1` is the *primary* path. No committed manifest
needs a non-Rust authoring source. The "v0 backdoor" framing in
the original chapter 01 §5.5 was aspirational — the field
shipped before any real authoring pipeline existed, and reality
caught up to the framing rather than the other way around.

## §3 Option set

### Option 1. Ship a minimal `figma_export_v1` against a concrete fixture

Pro: removes the deferral. Con: the chosen JSON shape is
dictated by whichever Figma export tool we benchmark against.
Without a real rlvgl-using project shipping a Figma → app.yaml
workflow, we'd be designing the contract against an example we
invented, which inverts the "round-trip evidence drives the
spec" discipline that produced chapters 02–04.

### Option 2. Ship a minimal `uml_widget_v1` against a hand-picked dialect

Pro: parser-only, no network, easier than Figma. Con: no
convergence signal yet from real authoring work — picking
PlantUML salt vs Mermaid vs custom is essentially arbitrary
today, and the wrong choice locks tooling lineage.

### Option 3. Close-with-deferral, name the unfreeze criteria (RECOMMENDED)

Pro: matches the precedent set by chapter 03 §6.10's "Don't
remove the backdoor before there's a real front door"
disposition; converts an informal "still open" item into a
structured deferral that future agents can act on without
re-deriving the question.

Con: leaves `rust_inline_v1` as the primary path indefinitely.
Acceptable: chapter 03 §6.10 already accepted this state;
APP-06-A merely formalises it.

## §4 Recommendation

Adopt **Option 3**. Reclassify both deferred formats as
**Coupled** per CLAUDE.md retrospective §5: each is blocked by
an external assumption that has not landed at v0.5 and would
need to be revisited with the new context when it does.

### 4.1 `figma_export_v1` — Coupled

**Named assumption:** *A Figma authoring pipeline exists
outside this repo that exports rlvgl-targetable layouts to a
stable JSON format with a documented input schema.* This
assumption is unmet at v0.5; no rlvgl-using project has shipped
a Figma workflow.

**Unfreeze criteria (both required):**

1. At least one rlvgl-using project ships a Figma → app.yaml
   workflow end-to-end (designer authors in Figma → exports
   JSON → orchestrator emits a buildable crate).
2. The exported JSON shape is stable across a 6-month review
   window (no breaking schema changes from the export tool of
   record), so locking it into the spec doesn't immediately
   require a v2.

**Resurrection-prevention note:** Do not ship a partial
`figma_export_v1` ahead of (1). Designing the JSON contract
against an invented example produces a contract that's wrong
for whichever real export tool eventually lands. The chakra-
tokens precedent (chapter 02 §7.6) is informative: that format
was specified against a real export from the softoboros theme
project, not invented in advance.

### 4.2 `uml_widget_v1` — Coupled

**Named assumption:** *The rlvgl widget surface has stabilised
enough that hand-authoring widget trees in Rust (the
`rust_inline_v1` path) becomes a real pain point relative to a
textual DSL.* This assumption is unmet at v0.5; the existing
six round-trip manifests' Rust layouts are 30–200 lines each
and read clearly.

**Unfreeze criteria (both required):**

1. The `rlvgl-widgets` API surface stabilises around a documented
   "primitive widget" set (the QT-03b widget mapping is
   suggestive but not yet ratified as the canonical set; chapter
   00 §10 cross-reference would extend this).
2. At least three candidate manifests independently arrive at
   very similar widget-tree shapes — i.e. there is a real
   shared vocabulary that a DSL would compress. Until that
   convergence signal appears, picking a UML dialect is
   premature.

**Resurrection-prevention note:** Do not pick a UML dialect by
fiat. PlantUML salt, Mermaid flowchart subset, and structurizr
container diagrams all carry different semantics; the wrong
choice locks the project into a tooling lineage with weak
escape paths. Wait for the convergence signal in (2).

## §5 Acceptance — same-day

This sub-letter ships with three same-day amendments:

1. **`docs/app-schema/README.md`**: drop the "Remaining v0.5
   work" callout; rephrase to "APP initiative open work: none
   — deferred items tracked in [`APP-06-A.md`](APP-06-A.md)."
2. **Chapter 01 §15**: 2026-05-04 AMENDMENT entry citing
   APP-06-A as the canonical deferral record. §5.5 prose
   tweaked to point at APP-06-A for the deferred-format
   criteria; the enum values themselves are unchanged.
3. **Chapter 03 §6.10**: 2026-05-04 AMENDMENT entry promoting
   the existing 🟢 CLOSED disposition to "deferred-Coupled per
   APP-06-A," cross-referencing the unfreeze criteria.

Code changes:

- `src/bin/creator/app.rs` line 2178 error message refreshed:
  the existing "land in APP-02c+" wording is stale (APP-02c is
  shipped; APP-02c+ doesn't apply). New message points at
  APP-06-A so users hitting `layout_format: figma_export_v1`
  get a pointer to the deferral analysis.

No grammar changes; no test changes (the validator's existing
acceptance of all three enum values + the orchestrator's
hard-error for the two deferred ones preserves both the
"grammar permits" and "implementation rejects" properties).

## §6 Reconciliation with adjacent invariants

- **Chapter 01 §5.5 enum values (`figma_export_v1`,
  `uml_widget_v1`, `rust_inline_v1`)**: unchanged. The grammar
  continues to accept all three so a future re-opening doesn't
  require a §5 amendment to the enum.
- **Chapter 02 §7.7 layout-translator contract**: unchanged.
  The implementation continues to handle `rust_inline_v1` and
  hard-error on the deferred formats; the error message points
  at APP-06-A for context.
- **Chapter 03 §6.10**: amended in §5 above. The 🟢 CLOSED
  disposition stands; APP-06-A formalises the deferral with
  named criteria.
- **APP-04 (state machines)**: orthogonal. SM-bearing manifests
  use `rust_inline_v1` for screens regardless of state-machine
  presence (verified: `examples/stm32h747i-disco/app-with-sm.yaml`
  carries `rust_inline_v1` for all four screens).
- **APP-05 (Cargo features graph)**: orthogonal. The feature-
  graph templates ship per-prong dependency tables; layout
  format is independent.

## §7 Non-goals

- Speculative implementation of `figma_export_v1` against an
  invented JSON shape (§4.1 resurrection-prevention).
- Picking a UML dialect by fiat without the convergence signal
  (§4.2 resurrection-prevention).
- Removing the enum values from chapter 01 §5.5. The enum
  preserves them as legitimate future formats; only their
  unimplemented status is being formalised.
- Renumbering or reflowing the chapter 03 §6.10 disposition.
  The existing 🟢 CLOSED entry is correct; APP-06-A amends it
  with a deferral classification, doesn't replace it.

## §8 Files cited

- [`docs/app-schema/01-manifest-schema.md`](01-manifest-schema.md)
  §5.5 (layout format enum) and §15 (change log target for
  the same-day amendment).
- [`docs/app-schema/02-generator-pipeline.md`](02-generator-pipeline.md)
  §7.7 (layout-translator contract).
- [`docs/app-schema/03-round-trip.md`](03-round-trip.md) §6.10
  (existing 🟢 CLOSED disposition; amended in §5 above).
- [`docs/app-schema/README.md`](README.md) — open-work block
  rewritten in §5 above.
- [`src/bin/creator/app.rs`](../../src/bin/creator/app.rs)
  line 86 (`LAYOUT_FORMATS` const) and line 2178 (error
  message refreshed in §5 above).
- [`CLAUDE.md`](../../CLAUDE.md) §"Initiative retrospective"
  §5 (Coupled / Safe / Abandoned classification).

## §9 Change log

| Date       | Status   | Note                                                                                                                                  |
| ---------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-05-04 | RESOLVED | Initial close-with-deferral. Both `figma_export_v1` and `uml_widget_v1` classified Coupled per CLAUDE.md retrospective §5. Unfreeze criteria named. README open-work block updated; chapter 01 §15 + chapter 03 §6.10 amended same-day to cite this analysis. |
