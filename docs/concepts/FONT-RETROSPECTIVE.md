<!--
FONT-RETROSPECTIVE.md — Initiative-completion retrospective for the
font-selection + anti-aliased-widget-text initiative (FONT-00..04).
Historical artifact. Behaviour PRs reference FONT-00 §15 + the canonical
sections directly, never this file.
-->

# FONT — Initiative Retrospective

**Initiative:** Font selection + anti-aliased widget text (FONT-00..04).
**Completion event:** FONT-01..04 all landed 2026-06-15; FONT-00 §12.A–§12.D
all boxed; §12.E (this retrospective + CHANGELOG + concepts README) closing.
**Canonical spec:** [FONT-00-CONCEPTS.md](FONT-00-CONCEPTS.md) (ratified
2026-06-14, §15 change log is authoritative).

Neutral, oriented at what to encode for the next structurally-similar
initiative (a small additive substrate riding on a larger completed one — here
LPAR-08 text). Per the CLAUDE.md "Initiative retrospective" discipline; §1–§7
shape, §8 change log. The first reference is
[DCB-RETROSPECTIVE.md](DCB-RETROSPECTIVE.md); this follows the same shape.

## 1. Outcome snapshot

Final architecture (all additive over the LPAR-08 shaped-text substrate; no
existing widget's default pixels changed):

- **Selection (FONT-01).** `core::font::WidgetFont(Option<&'static dyn
  FontMetrics>)` with `resolve()` → assigned handle or `FONT_6X10`. A uniform
  `set_font(&'static dyn FontMetrics)` on `Label`, `ui::Input`/`Textarea`/
  `FileBrowser`, and 21 `widgets::` widgets. No constructor or `Widget`-trait
  signature changed.
- **AA (FONT-02).** No new code path: a widget renders AA iff its resolved font
  returns multi-valued coverage (`PackedFont`). Proven by
  `widgets/tests/font_aa_conformance.rs` (partial-alpha through a real
  `blend_row`-overriding ARGB canvas; 1-bit-vs-AA contrast).
- **ArcLabel (FONT-03).** New defaulted `Renderer::draw_glyph(font, ch, origin,
  color)` single-glyph coverage helper; `ArcLabel` renders coverage along the
  arc instead of `draw_text`, adopting `WidgetFont`. Last legacy-`draw_text`
  widget closed. `config_menu.rs` close-"X" migrated.
- **Rotated throughput (FONT-04).** `RotatedRenderer::draw_glyph`/
  `draw_text_shaped` rotate a glyph's coverage once into a bounded
  `.rlvgl_blit_scratch` static and emit physical rows via `inner.blend_row`;
  zero-drift parity vs the software reference.

Deferred items (enumerated, classified in §5): the `FontId → handle`
theming registry; a core-resident AA font; the `ClipRenderer`-wrapped `Label`
rotated fast path; `EventWindow`/`draw_panel_header` `WidgetFont` conversion.

Residual risks, each with its riding assumption:

- **R1 — rotated `Label` text stays per-pixel.** FONT-04 accelerates only
  *direct* `draw_glyph`/`draw_text_shaped` callers (`ArcLabel`, `config_menu`).
  Assumption: on the disco widget path `Label` clip-wraps its renderer, so its
  glyph coverage still funnels through `ClipRenderer::blend_row` →
  `RotatedRenderer::blend_row` (per-pixel). Only bites if `Label`-heavy screens
  are drawn through `RotatedRenderer` at high glyph counts.
- **R2 — `font_id` silently inert.** `ResolvedStyle.font_id` exists but no
  widget honors it (§5.C). Assumption: callers select fonts only via
  `set_font`. Bites if a future contributor expects the style cascade's
  `font_id` to change rendered fonts.
- **R3 — shared scratch placement.** `.rlvgl_blit_scratch` now holds two
  statics. Assumption: consumers' linker scripts map it off the MSP
  stack-growth path (disco ERRATA-005). Bites a new consumer that copies the
  Rust but not the `SECTIONS` rule.

## 2. Divergence log

Load-bearing section. Each entry: **Assumption → Symptom → Root cause →
Detection gap.**

### D1 — The named work was already done ("Label migration" premise was stale)

- **Assumption.** The LPAR retrospective named "end-to-end glyph rendering +
  Label migration" as the next high-leverage item, implying `Label` still drew
  text through a backend-opaque / extent-only path needing migration.
- **Symptom.** Investigation before drafting found `Label` (and ~all widgets)
  already shaped text and fed real 1-bit coverage to pixels via
  `draw_text_shaped → draw_glyph_coverage → blend_row`. The pipeline was wired;
  nothing needed "migrating to render glyphs."
- **Root cause.** The pipeline landed incrementally across LPAR-08/11–15; the
  LPAR retrospective named the follow-up from the older mental model without
  re-verifying current `widgets/src` state. "Text rendering works" and "AA
  font *selection* works" were conflated.
- **Detection gap.** No fixture asserted "AA pixels reach the buffer," and 1-bit
  coverage *looks* correct, so the absence of selection/AA was invisible. The
  gap was caught only by reading the code before drafting — not by any gate.

### D2 — `FontId` was assumed not to exist

- **Assumption.** FONT-00 draft §5.C implied FONT would introduce font
  identity; `FontId` did not yet exist.
- **Symptom.** `FontId(pub u16)` (`core/src/font.rs:15`) and
  `ResolvedStyle.font_id` (`core/src/style_cascade.rs`) already existed, used
  by the style cascade.
- **Root cause.** The draft was written from a "selection is wholly absent"
  model without grepping `core::font` + `style_cascade`.
- **Detection gap.** Caught at the draft→review boundary (self-review during
  FONT-01), before any code rode the wrong assumption. The spec-before-code
  review gate worked as designed; corrected in FONT-00 §15 (2026-06-14).

### D3 — The AA fixture's DejaVu asset was unreachable

- **Assumption.** FONT-00 §6.C froze that the AA conformance fixture would use
  the disco example's DejaVu `PackedFont` assets.
- **Symptom.** Those assets — specifically the generated glyph table
  `DEJAVU_SANS_24_GLYPHS` — live in the *example* crate, unreachable from
  `widgets/tests` (the example depends on widgets, not vice-versa).
- **Root cause.** §6.C conflated "which real font for examples" with "which
  font for the host fixture"; the fixture's crate-location / dependency-
  direction constraint was not considered at freeze time.
- **Detection gap.** Surfaced only at implementation (FONT-02) when locating
  the asset. No earlier gate models "is this frozen test input reachable from
  the test's crate." Amended via FONT-02a (synthetic-but-real `PackedFont`,
  the `core/tests/font_metrics.rs` idiom) before coding the fixture.

### D4 — ArcLabel's `WidgetFont` adoption was mis-sequenced as "additive"

- **Assumption.** FONT-00 draft §12.A had every widget — `ArcLabel` included —
  adopt `WidgetFont` in FONT-01 as a purely additive change.
- **Symptom.** `ArcLabel`'s font also drives *advance geometry* (`Δθ =
  advance/r`) and had a no-font 8 px fallback with a colocated test asserting
  it. Adopting `WidgetFont` (no-font → `FONT_6X10`, advance 14 px) changes
  placement — a behavior change, not additive.
- **Root cause.** The draft treated `ArcLabel` like the coverage-only widgets,
  missing that its font is dual-purpose (geometry + coverage).
- **Detection gap.** Self-caught during FONT-01 sequencing; the colocated
  advance test would also have failed at test time. Moved to FONT-03 (§12
  amendment 2026-06-14) so FONT-01 stayed additive.

### D5 — `ClipRenderer` intercepts the rotated glyph fast path for `Label`

- **Assumption.** FONT-00 §2.4/§8 framed "the STM32H747I-DISCO widget path"
  as reaching `RotatedRenderer::blend_row` directly, so overriding
  `draw_text_shaped`/`draw_glyph` on `RotatedRenderer` would accelerate the
  widget glyph path.
- **Symptom.** `Label::draw_with_font` wraps its renderer in a `ClipRenderer`
  internally; `draw_text_shaped` therefore runs on `ClipRenderer` (trait
  default), whose per-glyph coverage loop calls `ClipRenderer::blend_row` →
  `RotatedRenderer::blend_row`. The `RotatedRenderer::draw_text_shaped` override
  is never reached for `Label`; it accelerates only direct callers (`ArcLabel`,
  `config_menu`).
- **Root cause.** §2.4's "widget path → `RotatedRenderer::blend_row`" framing
  did not trace the per-widget `ClipRenderer` wrapper that `Label` inserts; the
  interception layer was not followed end-to-end at freeze time. (Note `Label`
  *does* reach `RotatedRenderer::blend_row` — so §2.4 is correct that the cost
  lands there — but the fix point, `draw_text_shaped`, sits *above* the clip
  wrapper and is bypassed.)
- **Detection gap.** No test exercised a full `Label`-on-`RotatedRenderer` path;
  found by tracing the call chain during FONT-04 design. Mitigated by keeping
  the override correct for direct callers and documenting the limitation
  (FONT-00 §15, 2026-06-15); the clip-aware fix is deferred-Safe (§5).

## 3. Refactor points

Decision inflection nodes. Each: **Trigger → Alternatives → Selection rationale
→ Cost of switch.**

### RP1 — Reframe "Label migration" → the five real gaps

- **Trigger.** D1 (stale premise).
- **Alternatives.** (a) Declare the named item already done and close.
  (b) Reframe to the genuine gaps: selection, AA, ArcLabel, rotated throughput,
  AA fixture.
- **Selection rationale.** (b): the *intent* behind the named item — "make
  widget text good" — was unmet (no AA, no font choice, one legacy widget). A
  bare close would have left that intent stranded with no record of why.
- **Cost of switch.** A full FONT-00 §0–§15 concepts doc instead of a one-line
  close — but it converted a vague item into four independently-conformant
  phases.

### RP2 — Synthetic `PackedFont` over copying DejaVu into `widgets/tests`

- **Trigger.** D3 (asset unreachable).
- **Alternatives.** (a) Hand-authored synthetic `PackedFont` with intermediate-
  alpha glyph data. (b) Copy the DejaVu `.bin` + generated glyph table into
  `widgets/tests`. (c) Relocate the fixture to a crate that can reach disco
  assets.
- **Selection rationale.** (a): host-portable, deterministic, license-free,
  zero asset shipping, and the established repo idiom
  (`core/tests/font_metrics.rs`). A synthetic `PackedFont` *is* a real
  `PackedFont` exercising the real coverage path, satisfying §9.A's "real AA
  font" requirement. (b) ships a binary blob for no gain; (c) fragments fixture
  homes.
- **Cost of switch.** One docs amendment (FONT-02a) to §6.C/§9.C/§15.

### RP3 — `draw_glyph` as a defaulted `Renderer` method, not a free function

- **Trigger.** §7.B needs a public single-glyph coverage helper for `ArcLabel`.
- **Alternatives.** (a) Defaulted `Renderer::draw_glyph`. (b) Free function in
  `core::font`/`core::draw`.
- **Selection rationale.** (a): a trait method lets a backend
  (`RotatedRenderer`) *override* it for glyph-blit acceleration (FONT-04) —
  exactly what §8 needed. A free function (b) could not be overridden, so
  FONT-04's rotate-then-blit would have had nowhere to hook. Defaulted, so it
  breaks no existing `Renderer` impl, and `ClipRenderer` needs no override (the
  default routes through its own clipped `blend_row`).
- **Cost of switch.** One new defaulted trait method (additive).

## 4. Mitigation patterns

Abstracted, reusable. "When X + Y → apply Z."

- **MP1 — Re-verify a retrospective-named item against current code before
  drafting.** When an initiative is seeded by a *prior* retrospective's
  forward-looking item, the named gap may have closed incrementally since.
  Grep/read the target surface first; draft against reality, not the older
  mental model. (From D1.)
- **MP2 — Verify a frozen test input is reachable from its test's crate.** When
  a spec freezes an asset/fixture input, confirm the chosen asset is reachable
  from the fixture's crate *before ratifying* — dependency direction
  (`example → widgets`, not reverse) makes example assets invisible to
  `widgets/tests`. Prefer a synthetic-but-real input for host fixtures. (From
  D3, RP2.)
- **MP3 — A selection slot on a dual-purpose field is a behavior change.** When
  adding a `set_font`-style slot to a widget whose font *also* drives geometry
  (advance, layout), it is NOT additive; sequence it atomically with the
  geometry/render change and update the colocated geometry test. (From D4.)
- **MP4 — Trace the full wrapper chain before placing an override.** When
  accelerating a wrapped renderer, an override on an *inner* renderer is
  bypassed by an *outer* wrapper's defaulted method. Identify the outermost
  renderer the widget actually calls (here: `Label` → `ClipRenderer`); place
  the override there, or accept the inner path and document it. Treat the
  wrapper chain as an unstable boundary until traced end-to-end. (From D5.)
- **MP5 — Rotate-then-blit for orientation-wrapping renderers.** A horizontal
  coverage row maps to a vertical column under 90° rotation, so forwarding
  per-row to an inner row primitive is impossible. Instead rotate the whole
  coverage block once into a bounded scratch (landscape `(col,row)` →
  `scratch[col*h + (h-1-row)]`), then emit *physical* rows through the inner
  row primitive. Because the inner primitive does the same source-over, output
  is bit-identical — a pure throughput change verifiable by zero-drift parity.
  (From FONT-04; mirrors the existing `draw_pixels` + DMA2D `draw_glyph_rotated`
  pattern.)

## 5. Deferred-work reclassification

- **Safe** (orthogonal; no core-invariant impact):
  - Core-resident AA `PackedFont` (§6.C) — purely additive if it lands.
  - Clip-aware rotated coverage fast path for `Label` (the D5/R1 limitation) —
    a new code path; does not change existing pixels.
  - `EventWindow` / `draw_panel_header` `WidgetFont` conversion — already
    font-selectable via construction/parameter; conversion is polish.
- **Coupled** (revisit only with the named owner/context):
  - The `FontId → &'static dyn FontMetrics` registry / theming layer (§5.C) —
    **coupled to the LPAR-07 style/theme owner.** The assumption that must hold
    before reopening: a theming owner exists to decide how `resolved_style.
    font_id` resolves to a handle. Until then widgets MUST NOT honor `font_id`
    (R2). `WidgetFont` is the handle slot that resolution will target.
- **Abandoned** (explicitly killed; resurrection-prevention):
  - Routing widget text through the star-crawl A8 path
    (`widgets/src/motion/crawl`, `effect.rs::blend_a8_row_inline`). That path is
    crawl-specialized and bypasses `Renderer`; reusing it for widgets would
    fork the draw model into two incompatible glyph pipelines. **Do not
    revive** — it was evaluated and rejected in FONT-00 §14; the `Renderer`
    coverage path is the single widget glyph pipeline.

## 6. Forward constraints

The only normative section. Future planning docs treat these as binding.

- **FC1.** Do not start a "migration"/"finish X" initiative without
  re-verifying X's current state against the code. A prior retrospective's
  forward item is a hypothesis, not a fact. (D1/MP1.)
- **FC2.** A future `FontId` registry (call it FONT-05+) MUST be designed by /
  with the LPAR-07 style-theme owner and MUST NOT unilaterally wire widgets to
  honor `resolved_style.font_id`. `WidgetFont` + `set_font` remain the sole
  selection channel until that registry is ratified. (D2/R2/§5.C.)
- **FC3.** Any new orientation/clip wrapper that wants glyph (or other
  per-element) acceleration MUST intercept at the method the widget calls on the
  *outermost* renderer, having traced the wrapper chain; an inner override that
  an outer defaulted method bypasses is dead acceleration. (D5/MP4.)
- **FC4.** `.rlvgl_blit_scratch` is a shared, multi-static section (Color
  `SCRATCH` + u8 `SCRATCH_COV`). Any consumer copying the rotated-renderer code
  MUST also carry the `SECTIONS` rule mapping it off the MSP stack-growth path
  (disco ERRATA-005); adding a third static is fine but must respect the same
  placement. (R3.)

## 7. Provenance hooks

Outcome → issue → fix → evidence, one hop:

- **FONT-00 draft / ratify:** `a6eef12` (draft), `5cca5e4` (ratify + CLAUDE.md
  prefix/applicability + concepts README). Spec: FONT-00 §0–§15.
- **D1 (stale premise) → RP1 (reframe):** FONT-00 §1/§2/§15 (2026-06-14 draft
  entry); seeded by [LPAR-RETROSPECTIVE.md](LPAR-RETROSPECTIVE.md) §6/§7.
- **D2 (`FontId` exists):** FONT-00 §5.C / §10 / §15 (2026-06-14 accuracy
  correction). Code: `core/src/font.rs:15`, `core/src/style_cascade.rs`.
- **FONT-01 (selection):** `da29917` (slice 1: `WidgetFont` + `Label`),
  `0da9c3b` (slice 2: 21 widgets + `ui`). Spec: FONT-00 §5, §12.A.
- **D3 (DejaVu unreachable) → RP2 (synthetic font):** `4ed5c75` (FONT-02a docs).
  Spec: FONT-00 §6.C / §9.C / §15 (2026-06-15). Idiom:
  `core/tests/font_metrics.rs`.
- **FONT-02 (AA fixture):** `01458de`. Test:
  `widgets/tests/font_aa_conformance.rs`. Spec: FONT-00 §6, §9, §12.B.
- **D4 (ArcLabel sequencing) → RP3 (`draw_glyph` method) / FONT-03:** `6a61767`.
  Code: `core/src/renderer.rs` (`draw_glyph`), `widgets/src/arc_label.rs`,
  `examples/stm32h747i-disco/src/config_menu.rs`. Test:
  `widgets/tests/font_arc_label_coverage.rs`. Spec: FONT-00 §7, §12.C, §15
  (2026-06-14 sequencing + 2026-06-15 completion).
- **D5 (ClipRenderer interception) / FONT-04:** `d720b92`. Code:
  `platform/src/blit.rs` (`RotatedRenderer::draw_glyph`/`draw_text_shaped` +
  `blit_glyph_coverage_rotated`). Test:
  `platform/tests/font_rotated_glyph.rs`. Reference:
  `platform/src/dma2d_draw.rs:386` (`draw_glyph_rotated`); scratch placement
  per disco-analyzer ERRATA-005. Spec: FONT-00 §8, §12.D, §15 (2026-06-15).

## 8. Change log

- **2026-06-15** — FONT retrospective drafted at initiative completion
  (FONT-01..04 landed; FONT-00 §12.A–§12.D boxed). Captures D1–D5 divergences,
  RP1–RP3 refactor points, MP1–MP5 mitigation patterns, the Safe/Coupled/
  Abandoned deferred reclassification, and forward constraints FC1–FC4.
