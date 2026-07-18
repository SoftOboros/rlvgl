<!--
RATATUI-00-CONCEPTS.md — Ratatui text-graphics extension initiative.
-->

# RATATUI-00 — Curated Unicode, Anti-Aliased Text, and Style-Modifier Fidelity for `ratatui-rlvgl`

**Status: RATIFIED 2026-07-17** (§15). §5–§9 frozen decisions are normative;
RATATUI-00a/b/c/d are all unblocked — the companion SCTD-04 §7 amendment
required for RATATUI-00c (§8, §10) has already landed the same day.

New initiative family. Extends the `ratatui-rlvgl` backend and `RatatuiView`
widget shipped by [SCTD-04](SCTD-04-RATATUI-RLVGL-INTEGRATION.md) with the
capabilities that doc explicitly bypassed or deferred. Commit-subject prefix
`RATATUI-NN[a-z]:`.

The key words MUST, MUST NOT, SHALL, SHOULD, SHOULD NOT, MAY, and RECOMMENDED
are per RFC 2119 / 8174.

## §0 Authority policy

- Ratatui's `Backend`, `Terminal`, `Buffer`, `Cell`, `Modifier`, `Viewport`
  semantics are **as defined in `SoftOboros/ratatui` at
  `fc7c6a70794eebeaad5a1b732b9d5446dc9a4cb0` (tip of
  `origin/dev/ratatui-rlvgl-backend`); used without modification.**
  RATATUI-00 execution reopens this branch rather than cutting a new one —
  it was never merged upstream or tagged as a Ratatui release, so it is
  still the live integration branch for this crate.
- `RlvglBackend`, `RatatuiSurface`, `RatatuiView`, `CellMetrics`, and the
  `draw_cell`/`bitmap_font_fallback` rendering path are **as defined in
  `vendor/ratatui/ratatui-rlvgl/src/{backend.rs,view.rs}` at the SCTD-04
  baseline; adapted by this initiative** per §6–§8 below.
- `PackedFont`, `GlyphMetric`, and `FontMetrics` are **as defined in
  `core/src/{packed_font.rs,font.rs}`; used without modification.**
- The `WidgetFont` handle slot and `set_font`-style per-widget selection
  model are **as defined in [FONT-00-CONCEPTS.md](FONT-00-CONCEPTS.md) §5
  and `core/src/font.rs:191`; used without modification.** RATATUI-00 is a
  *consumer* of this model, exactly as `ArcLabel` is (FONT-00 §7) — it does
  not add a second selection mechanism.
- The `FontId → &'static dyn FontMetrics` cascade/theme registry is **as
  defined in [FONT-05-FONT-REGISTRY.md](FONT-05-FONT-REGISTRY.md); not
  used by this initiative.** `RatatuiView` is not a themed cascade node (it
  paints a retained cell surface, not a styled object tree), so §10
  reconciles this as explicitly out of scope rather than silently
  ignored.
- `rlvgl-creator fonts pack` is **as defined in `src/bin/creator/fonts.rs:32`;
  used without modification.** It already accepts an explicit `chars: &str`
  parameter — no new packing tool is needed for a curated codepoint subset.
- The tick-driven, no-wall-clock timing substrate is **as defined in
  [ANIM-00-CONCEPTS.md](ANIM-00-CONCEPTS.md); used without modification**
  where §8 below adopts it for blink.
- SCTD-04's redraw-only-on-change invariant (§7) and its documented v1
  limitations (§6.3, §6.4, §11, and the `ratatui-rlvgl` README's own
  "Version 0.1" paragraph) are **as defined in
  [SCTD-04-RATATUI-RLVGL-INTEGRATION.md](SCTD-04-RATATUI-RLVGL-INTEGRATION.md);
  adapted by this initiative** per §6–§9 and §10.

## §1 Purpose

SCTD-04 shipped a faithful, tested `ratatui-rlvgl` backend and named its own
gaps rather than papering over them. This initiative closes the gaps SCTD-04
explicitly flagged as follow-on work, structured so each is independently
ratifiable and independently shippable — no phase depends on another phase's
implementation, only (where noted) on its frozen decision:

1. **Curated Unicode repertoire + anti-aliased glyphs** (§6) — replace the
   ASCII-collapse fallback for box-drawing/block/arrow/status glyphs, and
   replace the 1-bit `FONT_6X10` default with `PackedFont` AA coverage,
   using infrastructure (`PackedFont`, `WidgetFont`, `fonts pack --chars`)
   that already exists in-tree but predates or postdates SCTD-04's scope.
2. **Real bold/italic** (§7) — replace the pixel-offset bold hack and the
   italic no-op with real font-variant selection, using the
   `DejaVuSansMono{,-Bold,-Oblique,-BoldOblique}.ttf` families already
   vendored at `assets/fonts/`.
3. **Blink fidelity** (§8) — decide whether blink stays a documented static
   approximation or gets a tick-driven redraw path, reconciling against
   SCTD-04 §7's frozen "redraw only on change" invariant.
4. **Scrollback / inline viewport** (§9) — re-affirm or reopen SCTD-04's
   non-goal now that the gap is enumerated explicitly rather than implied.

## §2 Problem statement (informative)

Evidence, pinned to the SCTD-04 baseline (`vendor/ratatui` @ `fc7c6a70`,
`rlvgl` @ `04326578`, tip of `origin/v0.2.6`, unreleased):

### 2.1 Unicode box-drawing collapses to ASCII

`bitmap_font_fallback` (`vendor/ratatui/ratatui-rlvgl/src/view.rs:344-353`)
maps every box-drawing character to `-`, `|`, or `+`, and every other
non-ASCII character to `?`. Ratatui's `Block` borders, `Gauge`, `Sparkline`,
`BarChart`, and most example apps render primarily through this glyph set —
today every one of them displays ASCII-art borders on `ratatui-rlvgl`
regardless of what the application author intended. SCTD-04 §6.4 named this
directly: "A future richer text-graphics revision SHOULD provide a curated,
embedded Unicode repertoire... Its codepoint set, flash cost, fallback
policy, and host/embedded parity MUST be specified and ratified before
implementation." That ratification has not happened; §6 below is it.

### 2.2 Text is always 1-bit, never anti-aliased

`draw_cell` (`view.rs:288,304`) calls `FONT_6X10.draw_char` directly — the
crate never touches the `WidgetFont`/`PackedFont` AA path that FONT-00
(ratified 2026-06-14, **after** SCTD-04's 2026-07-12 ratification) added
for the rest of the widget tree. This is not a design rejection; the AA
mechanism did not exist yet when SCTD-04 was scoped.

### 2.3 Bold is a redraw hack; italic and blink are no-ops

`view.rs:296-313` implements `Modifier::BOLD` by redrawing the same glyph
one pixel to the right — not a real weight, and it double-costs every bold
cell's draw budget. `Modifier::ITALIC`, `Modifier::SLOW_BLINK`, and
`Modifier::RAPID_BLINK` have no corresponding branch in `draw_cell` at all.
The crate's own README documents this as intentional v1 scope: "Italic and
blink modifiers are accepted but intentionally render as their non-animated
upright equivalents" (`vendor/ratatui/ratatui-rlvgl/README.md`).

### 2.4 Scrollback and inline viewport are explicitly unhandled

The same README: "terminal scrollback and inline `append_lines` behavior
are outside the initial bridge." `Viewport::{Fullscreen, Fixed, Inline}`
exist in the pinned `ratatui-core`
(`vendor/ratatui/ratatui-core/src/terminal/{init.rs,buffers.rs}`), and
`Backend::scroll_region_up`/`scroll_region_down` exist but are gated behind
the optional `scrolling-regions` Cargo feature
(`vendor/ratatui/ratatui-core/src/backend.rs:363,388`) — `RlvglBackend`
implements neither, relying on `Viewport::Fullscreen`/`Fixed` coverage only.

## §3 Canonical glossary

- **Curated repertoire** — *Owned by RATATUI-00 §6.* The frozen, explicit
  Unicode codepoint set packed into the terminal `PackedFont` family. Not
  "as much Unicode as fits" — an enumerated, ratified list.
- **Terminal `PackedFont` family** — *Owned by RATATUI-00 §6.* Four
  `PackedFont` assets (regular/bold/oblique/bold-oblique) packed from
  `assets/fonts/DejaVuSansMono{,-Bold,-Oblique,-BoldOblique}.ttf` restricted
  to the curated repertoire, at a cell size matching `CellMetrics`.
- **Fixed-advance rendering** — *Owned by RATATUI-00 §6.* The rule that
  every glyph in the terminal `PackedFont` family draws at exactly
  `CellMetrics::width()` horizontal step, ignoring the font's natural
  per-glyph `advance_fp16`. Required because Ratatui's cell-grid contract
  (one `Cell` per column, `unicode-width`-derived span) is incompatible
  with a truly proportional font; SCTD-04 §11 already named "no
  proportional-font terminal grid" as a non-goal and this initiative does
  not revisit it.
- **Blink phase** — *Owned by RATATUI-00 §8.* A tick-driven boolean
  carried on `RatatuiSurface`, advanced by an explicit caller-driven call
  (never a wall clock), that `draw_cell` reads to decide whether a
  `SLOW_BLINK`/`RAPID_BLINK` cell's foreground is currently visible or
  suppressed.

## §4 Source-of-truth map

| Concept | Single owner |
|---|---|
| Ratatui terminal/backend/viewport contracts | Ratatui baseline (§0) |
| `PackedFont`/`FontMetrics`/`WidgetFont` | `core/src/{packed_font.rs,font.rs}` (FONT-00, §0) |
| Font packing tool | `rlvgl-creator fonts pack` (§0) |
| Curated repertoire, terminal font family, fixed-advance rule | RATATUI-00 §6 |
| Bold/italic variant selection | RATATUI-00 §7 |
| Blink phase and redraw-trigger reconciliation | RATATUI-00 §8, jointly with SCTD-04 §7 |
| Scrollback/inline-viewport scope | RATATUI-00 §9, jointly with SCTD-04 §11 |
| Tick-driven timing substrate (if §8 adopts it) | ANIM-00 (§0) |
| SCTD demo composition, DP hero popup | unchanged, SCTD-04 §8 |

## §5 Frozen decision — branch and crate topology

Registration policy: **Standards Action**.

- `ratatui-rlvgl` source changes SHALL land on the reopened
  `SoftOboros/ratatui` branch `dev/ratatui-rlvgl-backend`, mirroring
  SCTD-04 §5's rule that backend commits land in the submodule branch
  first, then the outer `rlvgl` gitlink advances.
- The outer `rlvgl` side SHALL continue advancing the existing `v0.2.6`
  integration line (the submodule is presently a detached-HEAD checkout at
  the tip of `origin/v0.2.6`, not a separate topic branch) rather than
  cutting a new outer-repo branch, since `v0.2.6` has not been tagged or
  released yet.
  - `.gitmodules`' `branch = v0.2.5` pin for this submodule is stale
    against that; it was corrected to `v0.2.6` as a same-day housekeeping
    fix alongside this draft (§15).
- New terminal font assets SHALL be added under `assets/fonts/` (source
  TTFs, already present) and `examples/stm32h747i-disco/assets/fonts/`
  (packed `.bin`/`.json` outputs, mirroring the existing DejaVuSans
  proportional set's placement) — no new asset directory.
- No new crate. All RATATUI-00 code lands inside the existing
  `ratatui-rlvgl` crate boundary (`vendor/ratatui/ratatui-rlvgl/`); the
  MIT / `no_std` / `alloc`-only / no-terminal-backend-by-default
  constraints from SCTD-04 §5 remain unchanged and unamended.

## §6 Frozen decision — curated Unicode repertoire and anti-aliased rendering

Registration policy: **Standards Action** for the codepoint set itself
(SCTD-04 §6.4 already required this); **Specification Required** for
packing/build-script mechanics that don't change the codepoint set.

### §6.1 Terminal font family

- The terminal `PackedFont` family SHALL be packed via
  `rlvgl-creator fonts pack --chars <repertoire>` from
  `assets/fonts/DejaVuSansMono.ttf`,
  `assets/fonts/DejaVuSansMono-Bold.ttf`,
  `assets/fonts/DejaVuSansMono-Oblique.ttf`, and
  `assets/fonts/DejaVuSansMono-BoldOblique.ttf` (all already vendored,
  Bitstream Vera/DejaVu license, already redistributed elsewhere in this
  repo). No new font asset acquisition is required.
- Pack size SHALL be chosen so the family's cell metrics are pixel-exact
  at the existing `CellMetrics::font_6x10()` 12×20 geometry, or a new
  `CellMetrics` constructor SHALL be added if DejaVu Mono's natural
  proportions round better at a different integer cell size. Either way
  `CellMetrics` stays the single source of cell geometry (§6.2 of SCTD-04
  is unchanged: `RlvglBackend::window_size` still reports the exact pixel
  product).
- All four variants SHALL pack the identical codepoint set (§6.3), so a
  bold or italic box-drawing border never silently falls back to ASCII
  while its regular-weight neighbor renders correctly.

### §6.2 `WidgetFont` adoption

- `RatatuiView` SHALL gain a `WidgetFont`-shaped selection slot (or four,
  per §7), defaulting to the bundled terminal `PackedFont` family rather
  than `FONT_6X10`, consistent with FONT-00 §5's "defaults to `FONT_6X10`
  when unset" pattern generalized to a non-default explicit assignment.
  `FONT_6X10` SHALL remain available as an explicit opt-out for callers
  that want the deterministic 1-bit baseline (e.g. the lowest-flash-cost
  target profile).
- `draw_cell` SHALL route glyph drawing through `FontMetrics::shape` /
  `glyph_coverage_row` (the existing LPAR-08/FONT-00 pipeline —
  `core/src/renderer.rs` `draw_text_shaped` → `draw_glyph_coverage` →
  `blend_row`) rather than calling `FONT_6X10.draw_char` directly, with
  the fixed-advance override from §6.1/glossary applied at the call site.
  No new `Renderer` method is introduced (mirrors FONT-00 §0's "FONT does
  NOT add a new `Renderer` text method" constraint).

### §6.3 Codepoint set (ratified)

- **Base**: the existing ASCII 0x20–0x7E baseline (already correct today).
- **Box drawing**: the full U+2500–U+257F block (light/heavy/double lines,
  corners, tees, crosses, dashed variants) — the `bitmap_font_fallback`
  table already lists every one of these codepoints as degraded, so
  including the whole block rather than a subset avoids moving the
  "some box-drawing glyphs still degrade" problem instead of closing it.
- **Block elements**: U+2580–U+259F (used by `Gauge`, some `Sparkline`
  renderers, and block-styled progress indicators).
- **Arrows**: the full U+2190–U+21FF block (not just the basic ↑↓←→
  quartet) — owner-confirmed 2026-07-17.
- **Braille patterns (U+2800–U+28FF): excluded from this pass.**
  Owner-confirmed 2026-07-17. Ratatui's `Canvas` braille marker mode and
  braille-glyph spinners therefore continue to render through the
  existing `bitmap_font_fallback` degrade path (non-ASCII → `?`) on
  `ratatui-rlvgl`, unchanged from today. Reopening this exclusion
  requires a named first consumer and a §15 amendment, per the family's
  reopen convention (§9's own precedent).
- **Status/symbol set**: an explicit list — ●○■□✓✗▪▸ — rather than an
  open-ended "useful symbols." SCTD-04 §6.4's own phrase was "useful
  status symbols," which is not itself an enumerable set; this list is
  RATATUI-00's concrete resolution of that phrase.
- Every codepoint outside this frozen set SHALL continue to fall back
  through the existing `bitmap_font_fallback` policy (ASCII substitution,
  then `?`) — this initiative narrows what needs to degrade, it does not
  remove the fallback path.
- Flash cost (packed `.bin` size × 4 variants) and host/embedded parity
  (identical asset on host and STM32H747I-DISCO, per SCTD-04 §6.4's own
  requirement) SHALL be measured and recorded in §15 during RATATUI-00a
  execution, before §12 acceptance. Excluding Braille (256 codepoints,
  the single largest candidate block) was the deliberate lever for
  keeping that flash cost bounded in this pass.

## §7 Frozen decision — real bold and italic

Registration policy: **Specification Required** (additive, no cross-phase
enum coupling — this only changes which already-defined `PackedFont`
variant `draw_cell` selects).

- `draw_cell` SHALL select among the four §6.1 variants by
  `(cell.modifier.contains(Modifier::BOLD),
  cell.modifier.contains(Modifier::ITALIC))`, replacing both the
  pixel-offset bold hack (`view.rs:296-313`) and the italic no-op.
- The existing `Modifier::DIM` (`dim()` in `color.rs`), `REVERSED`,
  `HIDDEN`, `UNDERLINED`, and `CROSSED_OUT` handling in `draw_cell` is
  unchanged by this phase; they compose with whichever of the four
  variants is selected.
- No new modifier semantics are introduced. `Modifier::BOLD` selects the
  bold-weight glyph outline instead of a synthetic double-draw;
  `Modifier::ITALIC` selects the oblique-slant glyph outline instead of
  rendering upright. Both are real font-variant selection, not visual
  approximation, so this phase resolves SCTD-04 §6.3's "MAY degrade to
  documented static approximations" language into "does not degrade" for
  these two modifiers specifically.

## §8 Frozen decision — blink

Registration policy: **Standards Action** (this decision amends SCTD-04
§7, a cross-phase frozen invariant).

**Owner-ratified 2026-07-17: tick-driven blink phase (Option B).**

- `RatatuiSurface` SHALL carry a blink-phase boolean, advanced only by an
  explicit caller call (e.g. `RatatuiSurface::advance_blink_phase()`) —
  never a wall clock, `Instant`, or OS timer. Consuming applications wire
  it to whatever tick source they already own (the SCTD demo's existing
  Auto-tick cadence is the reference first consumer), consistent with the
  ANIM-00 no-wall-clock precedent (§0).
- `draw_cell` SHALL read the current blink phase when a cell carries
  `Modifier::SLOW_BLINK` or `Modifier::RAPID_BLINK`, suppressing the
  glyph (rendering only the resolved background) on the "off" phase and
  drawing normally on the "on" phase. `SLOW_BLINK` and `RAPID_BLINK`
  SHALL share one phase boolean in v1 (no independent slow/rapid
  cadence) — the distinction is available for the caller's tick-mapping
  choice, not a second internal counter.
- Advancing the blink phase SHALL count as a dirty-triggering event: it
  marks the full surface dirty (blink is a global, not per-cell, toggle
  in v1) and therefore composes with `RatatuiSurface::generation`/
  `dirty_cells` exactly like any other backend write.
- **Companion amendment — landed 2026-07-17.** SCTD-04 §7's "SHALL render
  a new Ratatui frame only when its snapshot, event log, focus, cursor,
  or bounds changes" invariant now also names "or blink phase" as a
  trigger, with a dated SCTD-04 §15 entry citing this section as the
  origin, per CLAUDE.md's stealth-revert prohibition (the amendment
  landed with this ratification, not silently after RATATUI-00c code).

## §9 Frozen decision — scrollback and inline viewport (re-affirmed non-goal)

Registration policy: **Standards Action**.

- `RlvglBackend` SHALL continue to support only `Viewport::Fullscreen` and
  `Viewport::Fixed`. `Viewport::Inline` and the `scrolling-regions`
  feature's `scroll_region_up`/`scroll_region_down` methods remain
  unimplemented.
- Rationale: `Viewport::Inline` exists to let a TUI app coexist with
  scrollback *above* it in a real ANSI terminal — printing content that
  scrolls up and out of the live region while preserving history in the
  terminal's own scrollback buffer. An embedded display has no such
  "history surface" to scroll content into; there is nothing for
  `Viewport::Inline` to be correct *about* on this backend. This is not a
  missing feature so much as a viewport mode without a target.
- If a concrete consumer needs partial-region scrolling (e.g. a log pane
  inside a larger fixed layout) that is `Rect`-scoped `Buffer` diffing
  Ratatui already does for you inside a `Viewport::Fixed` region — no
  backend-level scroll primitive is needed for that case either.
- Reopening this non-goal requires a named first consumer and a §15
  amendment, per the family's own reopen convention (mirrors DCB's
  "reopen with a named first user" pattern, `docs/concepts/README.md`).

## §10 Reconciliation with adjacent primitives

- **vs. SCTD-04 §6.3/§6.4**: those sections explicitly anticipated this
  initiative ("a future richer text-graphics revision," "MAY degrade to
  documented static approximations") — §6/§7 here are the promised
  follow-on, not a reversal. §6.4's codepoint-set/flash-cost/fallback/
  parity requirements are satisfied by §6.3's explicit list plus the §12
  acceptance gate.
- **vs. SCTD-04 §7**: amended, landed 2026-07-17. §8 ratified the
  tick-driven blink-phase option; SCTD-04 §7's normative text and §15
  both carry the amendment, citing this section as the cause, per
  CLAUDE.md's stealth-revert prohibition.
- **vs. FONT-00/FONT-05**: `RatatuiView` becomes a `WidgetFont` consumer
  (FONT-00 §5) exactly like `ArcLabel`. It does NOT participate in the
  FONT-05 cascade/`FontRegistry` — `RatatuiView` paints a retained cell
  surface it does not own the styling of (the surface's cells carry their
  own per-cell Ratatui `Style`, which is a different, pre-existing
  concept from rlvgl's `StylePatch` cascade). This is a deliberate
  non-adoption, recorded here so a future reader does not assume it was
  overlooked.
- **vs. ANIM-00**: engaged because §8 ratified the tick-driven blink
  phase; that phase advance is a caller-driven tick, not a new animation
  primitive — RATATUI-00 does not add a `Tween` or register with
  `Animations`, it just gates a boolean on an externally-driven call.
- **vs. LPAR-08's text substrate**: `draw_text_shaped`/`draw_glyph_coverage`
  /`blend_row` remain the single glyph-coverage pipeline; §6.2 explicitly
  routes through it rather than adding a second one, matching how FONT-00
  itself was scoped.

## §11 Non-goals

- No ANSI parser, PTY, shell, terminal emulator, or process launcher
  (unchanged from SCTD-04 §11).
- No dependency on a C library, FFI display backend, or non-Rust font
  library in the conforming hardware path (unchanged from SCTD-04 §11).
- No proportional-font terminal grid — §6's fixed-advance rule is
  mandatory, not a default that can be turned off per call site.
- No promise that every Unicode grapheme exists in the embedded font —
  §6.3 is an enumerated, finite repertoire; anything outside it still
  degrades via the existing fallback policy.
- No bidi shaping (unchanged from SCTD-04 §11).
- No terminal scrollback or inline viewport (§9, re-affirmed).
- No new state machine, no SCTD demo behavior change, no Media Player
  screen — this initiative is scoped entirely to `ratatui-rlvgl` and
  `RatatuiView`; the SCTD Dining Philosophers hero popup (SCTD-04 §8)
  gets improved rendering for free once its `RatatuiView` picks up the
  new default font, but its widget composition is unchanged.
- No publication or upstream Ratatui PR until this initiative's phases
  pass their own acceptance gates — mirrors SCTD-04 §11's own publication
  gate.

## §12 Acceptance checklist (normative)

A conforming RATATUI-00 implementation MUST satisfy the gates of every
phase it claims:

- **RATATUI-00a (curated repertoire + AA, §6):**
  - [ ] §6.3's codepoint set is ratified (§15 dated entry) before any
        packing commit lands.
  - [ ] All four terminal `PackedFont` variants pack the identical
        ratified codepoint set; flash-size and host/embedded-parity
        measurements are recorded in §15.
  - [ ] `draw_cell` renders through the shared `FontMetrics` coverage
        pipeline with fixed per-cell advance; a golden/pixel test proves
        a full-width box-drawing border renders unbroken glyphs, not `-`/
        `|`/`+`.
  - [ ] Glyphs outside the ratified repertoire still degrade through the
        existing `bitmap_font_fallback` policy (no silent regression).
  - [ ] `FONT_6X10` remains selectable as an explicit low-flash-cost
        opt-out.
- **RATATUI-00b (bold/italic, §7):**
  - [ ] `Modifier::BOLD`/`Modifier::ITALIC` select real font-variant
        outlines; the pixel-offset bold hack is removed.
  - [ ] A bold or italic box-drawing border (e.g. a `Block` with
        `BOLD`-styled title) renders with unbroken glyphs, proving §6 and
        §7 compose.
- **RATATUI-00c (blink, §8):**
  - [x] SCTD-04 §7's invariant carries a dated §15 amendment naming this
        initiative — landed 2026-07-17, ahead of any blink-phase code.
  - [ ] Blink-phase advance is caller-driven only; no wall clock,
        `Instant`, or OS timer appears in `ratatui-rlvgl`.
  - [ ] Advancing the blink phase marks the full surface dirty and is
        observable through the existing `generation`/`dirty_cells` API
        with no new surface-state accessor required for that purpose.
- **RATATUI-00d (scrollback non-goal, §9):**
  - [ ] `Viewport::Inline` and `scrolling-regions` remain unimplemented,
        documented as an intentional non-goal in the crate README (not a
        silent gap).
- **All phases:** `cargo fmt --all -- --check`, relevant clippy/tests,
  `./scripts/pre-commit.sh`, and independent Ratatui-repo build (no
  rlvgl-superproject patch) per SCTD-04 §12's precedent, pass for every
  touched crate.

## §13 Files cited and expected touch points

- `vendor/ratatui/ratatui-rlvgl/src/{backend.rs,view.rs,color.rs,lib.rs}`
- `vendor/ratatui/ratatui-rlvgl/README.md`
- `core/src/{packed_font.rs,font.rs,bitmap_font.rs,renderer.rs}`
- `src/bin/creator/fonts.rs`
- `assets/fonts/DejaVuSansMono{,-Bold,-Oblique,-BoldOblique}.ttf`
- `examples/stm32h747i-disco/assets/fonts/`
- `docs/concepts/{SCTD-04-RATATUI-RLVGL-INTEGRATION.md,FONT-00-CONCEPTS.md,FONT-05-FONT-REGISTRY.md,ANIM-00-CONCEPTS.md}`
- `.gitmodules` (branch pin correction, landed same day as this draft)

## §14 Unblocks

This ratification unblocks, in dependency order:

1. **RATATUI-00a:** font packing against the ratified §6.3 codepoint set
   (box drawing, block elements, full arrow block, status symbols; no
   Braille this pass), `WidgetFont`/AA adoption in `draw_cell`.
2. **RATATUI-00b:** bold/italic variant selection (may proceed in
   parallel with RATATUI-00a once §6.1's four source TTFs are packed,
   since it only changes variant *selection*, not the coverage pipeline).
3. **RATATUI-00c:** blink phase — the companion SCTD-04 §7 amendment
   (§8, §10) is already landed, so this may proceed independent of
   RATATUI-00a/00b.
4. **RATATUI-00d:** documentation-only — re-affirm the scrollback/inline
   non-goal in the crate README (may land any time, including before
   RATATUI-00a).

## §15 Change log

- 2026-07-17 — **DRAFT.** Initial RATATUI-00 concept gate. Proposed a
  curated Unicode repertoire + `PackedFont` AA adoption (§6, codepoint set
  not yet ratified), real bold/italic via existing vendored
  `DejaVuSansMono` variants (§7), a blink-fidelity decision between two
  options (§8, not yet chosen), and a re-affirmed scrollback/inline-
  viewport non-goal (§9). Same-day housekeeping: corrected the stale
  `.gitmodules` `branch = v0.2.5` pin for this submodule to `v0.2.6`
  (cosmetic — the submodule was already checked out at the `v0.2.6`
  branch tip by SHA).
- 2026-07-17 — **RATIFIED.** Owner resolved all three open questions from
  the draft: Braille (U+2800–U+28FF) excluded from this pass (§6.3);
  arrow coverage widened to the full U+2190–U+21FF block, not the minimal
  quartet (§6.3); blink resolved to Option B, the tick-driven blink-phase
  design (§8). The companion SCTD-04 §7 amendment landed in the same
  change (§10, §14). RATATUI-00a/b/c/d are all unblocked.
