<!--
QT-MEDIA-PLAYER-RETROSPECTIVE.md — initiative retrospective for the
QML→rlvgl media-player ingest effort (2026-06-26 → 2026-06-27).
-->

# QT Media-Player Ingest — Initiative Retrospective

Scope: driving the scjson tutorial `SkodaBoleroInfotainment` media-player frame
(`Qml/Media/FrameMedia.qml`) end-to-end through `rlvgl-creator qt emit
--target rlvgl` and onto real hardware (FireBeetle ESP32-P4 + DFR0550 panel),
treating every ingest defect as a bug to fix **in the emitter**, not to route
around in hand-written glue. The guiding rule throughout: **work *through* the
pipeline, not *around* it.**

This retrospective follows the §1–§7 shape from the repo's Spec-Before-Code
"Initiative retrospective" convention.

## §1 — Outcome snapshot

`rlvgl-creator`'s QML ingest now composes the Bolero media-player frame into a
recognisable control surface that renders on the panel: the Bolero background,
the `15 °C` header text, and a full transport bar — repeat ▸ rewind ▸ play ▸
forward ▸ shuffle — correctly sized, spaced, centred, and with clean icon
transparency.

`QT_EMIT_VERSION_RLVGL` advanced `13 → 17` across the effort. Emitter changes
live in `src/bin/creator/qt.rs` (+ `compress.rs`/`cli.rs` for the asset path).
The demo mounts at `examples/apps/sctd-demo` (`media_player_gen.rs` emitted,
`qt_assets.rs` + `assets/bolero/*.rle` vendored, `media_player_skin.rs`
wrapper). The IDF firmware payload is `examples/beetle-esp32p4-idf`.

**Deferred (explicitly, see §5):** reactive icon swap (Play/Pause, repeat &
shuffle modes), track-title/time/temperature text, album art, gradient/theme
colour fills, the header's content-driven height, and the `DimResolver`
local-bounds approximation. All are **state-binding / content** concerns, not
layout — the natural seam to the next initiative (scxml→istate→rlvgl).

**Residual risks:** (a) the magenta `#FF00FF` transparency key assumes the key
colour is absent from artwork — true for this corpus, not guaranteed in
general. (b) Repeater expansion assumes 48px-square icons and a literal-array
model. (c) `panelHeight` evaluates against local bounds, so deeply-nested
root-property references are approximate.

## §2 — Divergence log (assumption → symptom → root cause → detection gap)

The load-bearing section. Every visible defect traced to one shared anti-pattern
plus a handful of QML-semantics gaps.

1. **Opaque-white default backgrounds.**
   *Assumption:* widgets lowered from structural QML nodes are visually inert.
   *Symptom:* the whole media slot rendered **all-white** on the panel (DP slot
   fine). *Root cause:* rlvgl `Style::default().bg_color` is opaque white;
   every `Item`/`Row`/`Repeater`-fallback/`Label`/`Button` that lowered to a
   bare container painted a full-bounds white box, burying the background image
   drawn first. *Detection gap:* the host conformance test drew through
   `NullRenderer` (no pixels), so it asserted "doesn't panic," never "isn't
   white." **A renderer that doesn't produce pixels can't catch a rendering
   bug.**

2. **Unresolved layout dimensions collapse the tree.**
   *Assumption:* the anchor solver resolves `height: pane.panelHeight`.
   *Symptom:* every header/control row had height 0 or negative → nothing drew.
   *Root cause:* `panelHeight = height/6 - AppConsts.i_DISPLAY_PADDING` is a
   root property over a JS constant; unresolved, `default_h` fell back to the
   full parent extent, and an anchored sibling computed against the wrong
   extent went to zero, cascading. *Detection gap:* no test rendered a real
   layout and inspected per-node bounds; the per-node bounds dump was built
   ad-hoc during diagnosis.

3. **Unsized `Image` stretches to parent.**
   *Assumption:* an `Image` with no size uses its source size.
   *Symptom:* the mute icon stretched to 714×392 off-screen-left ("left strip").
   *Root cause:* `default_w`/`default_h` fell to `bounds.*` for unsized images;
   QML sizes an Image to its `sourceSize`. *Detection gap:* same as #2.

4. **`implicitWidth` ignored.**
   *Assumption:* explicit `width` is the only size source.
   *Symptom:* standalone buttons (`implicitWidth: 65`) rendered full-width →
   squished green band. *Root cause:* the solver read only `width`/`height`.

5. **RGB565 drops alpha.**
   *Assumption:* the RLEC asset path preserves icon transparency.
   *Symptom:* white boxes behind every transparent icon. *Root cause:* RLEC is
   RGB565 — no alpha channel; `rgb565_to_rgba` hardcodes `0xFF`. The operator
   identified this directly ("our rlvgl formats dropped some alpha?").
   *Detection gap:* the asset-vendoring step had no transparency conformance
   check; opaque-matte icons happened to blend on the dark strip and masked it.

6. **`Repeater` not instantiated.**
   *Assumption:* a `Repeater` over a model produces N children.
   *Symptom:* the centre transport buttons were absent. *Root cause:* the
   emitter left `Repeater` as an empty fallback container; the model array and
   delegate were parsed into the IR but never expanded.

## §3 — Refactor points (trigger → alternatives → selection → cost)

- **Diagnosis method.** *Trigger:* a fix to the opaque-white default produced a
  byte-identical render. *Alternatives:* keep theorising vs. instrument.
  *Selected:* a per-node "render this widget alone, count white/art pixels"
  tree-walk that named each culprit (fallback `Repeater` → non-literal-colour
  node → labels → gradient `Rectangle`). *Cost:* a throwaway test per round;
  paid back immediately — every subsequent fix was target-confirmed, not
  guessed. **Lesson: build the attribution probe before the third hypothesis.**

- **Transparency strategy.** *Trigger:* RGB565 has no alpha. *Alternatives:*
  (a) extend the codec to RGBA — large blast radius across all RLE consumers;
  (b) chroma-key a fixed colour like white — erases white icon pixels;
  (c) magenta sentinel keyed at encode + decode. *Selected:* (c) — contained to
  a new `--transparent-key` flag + the `qt_image` helper I own; zero shared-codec
  change. *Cost:* re-vendor 13 icons; binary (≤1-bit) edges.

- **Repeater layout.** *Trigger:* synthetic icons needed positioning.
  *Alternatives:* literal x-offsets (needs runtime width) vs. sibling anchors.
  *Selected:* sibling anchors (`__rep_btn_i.left = __rep_btn_{i-1}.right`),
  which reuse the existing solver's `verticalCenter` + `<id>.right` handling and
  centre the group. *Cost:* synthetic ids; faithful RowLayout-ish spacing.

## §4 — Mitigation patterns (reusable)

- **Default-to-invisible for the unresolved.** When the emitter can't resolve a
  fill/size/colour, prefer the **inert** default (transparent / natural-size),
  never an arbitrary opaque one. An opaque guess *buries* content; a transparent
  one *reveals* the gap. This single rule fixed defects #1, #3, and the
  gradient/theme `Rectangle` and no-source `Image` cases.
- **Pixel-level conformance, not panic-level.** Any "does it render" gate MUST
  draw into a real buffer and assert on pixels (histogram or per-node
  attribution). `NullRenderer` gates are necessary but not sufficient.
- **Fold capability gaps into a sentinel at the boundary you own.** When a
  shared format lacks a channel (alpha), encode it into a value the format
  *does* carry and decode it in your own consumer — avoid widening the shared
  format.
- **Expand declarative constructs in a pre-pass over the IR.** `Repeater`
  expansion, like component inlining, is cleanest as an IR transform before
  emit, so the normal solver/emit path handles the synthesised nodes uniformly.

## §5 — Deferred work reclassification

- **Coupled (revisit with state-machine context) — the next initiative:**
  reactive icon swap (Play/Pause, repeat/shuffle/mute modes), track-title /
  time / temperature text, album art. These read `scxmlBolero.*` predicates and
  a track datamodel; they are **binding**, not layout, and MUST be done by
  wiring real machine state (scxml→istate→rlvgl), not by hardcoding branches.
  The current static choices (first-/else-branch icon, `15 °C` placeholder) are
  scaffolding to be *replaced*, not extended.
- **Coupled (needs a colour model):** gradient/theme-colour fills
  (`AppConsts.cl_*`, `#AARRGGBB`). The translucent-white panel intent is
  currently approximated as transparent.
- **Safe (orthogonal):** the header's content-driven height (unsized `Item` →
  content vs. full-parent); the `DimResolver` root-extent threading (replace the
  local-bounds approximation). Neither blocks the state-binding work.
- **Abandoned:** none. (Do not "fix" transparency by chroma-keying white — see
  §3; that erases legitimately-white icon pixels.)

## §6 — Forward constraints (binding on the next initiative)

1. **Work through, not around.** scxml→istate→rlvgl integration MUST drive the
   real state machine into the emitted bindings. Do not hand-edit
   `media_player_gen.rs` or hardcode predicate branches in glue; if ingest
   can't express a binding, fix the emitter.
2. **Replace the scaffolding.** The static first/else-branch icon selection and
   placeholder text are explicit stand-ins for reactive bindings — wiring real
   state MUST remove them, not layer on top.
3. **Add a pixel-level render gate before extending.** A committed test that
   renders the media slot into a buffer and asserts a white/art histogram (or
   per-node attribution) MUST exist before further visual work, so regressions
   surface in CI, not at the bench.
4. **Keep the asset-transparency invariant.** Any new icon MUST be vendored
   with `--transparent-key` (or a successor), and the magenta sentinel MUST stay
   absent from artwork.

## §7 — Provenance hooks

- Emitter: `src/bin/creator/qt.rs` (`DimResolver`, `expand_repeaters_in`,
  `emit_qt_image_helper`, `solve_child_bounds`, the `WidgetKind::Button`/
  `Container`/`Image`/`Fallback` arms); asset path `compress.rs` + `cli.rs`
  (`--transparent-key`); codec `rlvgl-decomp/src/lib.rs` (RGB565 limitation).
- Spec: `docs/qt-support/03b-rlvgl-widget-mapping.md` §15 (v14→15, v16→17);
  `docs/qt-support/03c-anchor-resolver.md` §15 (amendments #4 dimension
  resolver, #5 content-size fallbacks).
- Demo + bench: `examples/apps/sctd-demo/` (`media_player_gen.rs`,
  `media_player_skin.rs`, `qt_assets.rs`, `assets/bolero/`);
  `examples/beetle-esp32p4-idf/` (IDF payload). Source QML:
  `streamz/submodules/scjson` (read-only) `…/Qml/Media/FrameMedia.qml`,
  `HeaderPanel.qml`, `MediaFunctionKeysPanel.qml`, `SelectButton.qml`,
  `AppConstants.js`.

## §8 — Change log

| Date       | Change                                                        |
| ---------- | ------------------------------------------------------------- |
| 2026-06-27 | Drafted at the close of the QML media-player ingest effort (emit v13→17), ahead of the scxml→istate→rlvgl integration initiative. |

---

MIT-licensed: MIT.
