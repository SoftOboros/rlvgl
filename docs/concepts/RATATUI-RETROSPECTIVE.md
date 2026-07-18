<!--
RATATUI-RETROSPECTIVE.md — RATATUI-00 initiative-completion retrospective.
-->

# RATATUI — Initiative Retrospective

**Drafted 2026-07-18.** Covers the RATATUI-00 initiative
([RATATUI-00-CONCEPTS.md](RATATUI-00-CONCEPTS.md)) from draft (2026-07-17)
through ratification and implementation of all four sub-phases
(RATATUI-00a/00a-wiring/00b/00c/00d) the same day. Per CLAUDE.md's
"Spec-Before-Code Planning Discipline → Initiative retrospective," this
file is a historical artifact; behavior PRs reference RATATUI-00-CONCEPTS.md
and its §15 change log directly, never this file.

## §1 Outcome snapshot

`ratatui-rlvgl` (`SoftOboros/ratatui`, `dev/ratatui-rlvgl-backend`
@ `adc75755`) gained:

- `ratatui-rlvgl/src/fonts.rs` — four crate-local `PackedFont` statics
  (`TERMINAL_REGULAR`/`BOLD`/`OBLIQUE`/`BOLD_OBLIQUE`), packed from
  `DejaVuSansMono{,-Bold,-Oblique,-BoldOblique}.ttf` at a curated
  375-codepoint repertoire (ASCII, full box drawing, block elements, full
  arrow block, 8 status symbols), embedded via `include_bytes!`, 166,088
  bytes total.
- `CellMetrics::packed_terminal()` — a new 14×21 (baseline 16) cell
  geometry, alongside the pre-existing `font_6x10()` 12×20.
- `RatatuiTerminalFont` (`Bitmap6x10` | `Packed{regular,bold,oblique,
  bold_oblique}`) and `RatatuiView::set_font_family` — the curated family
  is now the default; `Bitmap6x10` reproduces the pre-RATATUI-00 behavior
  byte-for-byte as an explicit, regression-tested opt-out.
- `draw_cell` split into `draw_cell_bitmap6x10` (legacy, unchanged) and
  `draw_cell_packed` (curated-first glyph lookup, `bitmap_font_fallback`
  only when genuinely out-of-repertoire, fixed `metrics.width()` advance
  per glyph via `Renderer::draw_glyph`, bold/italic selection by
  `(Modifier::BOLD, Modifier::ITALIC)`).
- `RatatuiSurface::advance_blink_phase()` — caller-driven, no wall clock;
  flips a shared phase, marks the surface fully dirty, unpublishes.
- A companion amendment to SCTD-04 §7's redraw-only-on-change invariant,
  naming blink-phase advance as a trigger.

**Deferred, explicitly:** Braille (U+2800–U+28FF) — excluded this pass,
reopen-gated on a named consumer per RATATUI-00-CONCEPTS.md §6.3/§9.
Scrollback/`Viewport::Inline` — re-affirmed permanent non-goal, not a
deferral.

**Known residual risks** (see §5 for handling):

- Two of 375 curated codepoints (✓ U+2713, ✗ U+2717) have no real glyph in
  the Oblique/BoldOblique source TTFs; `fontdue` rasterizes `.notdef` for
  those two, in those two variants only. `PackedFont::glyph()` still
  returns `Some` for them — no test or runtime signal distinguishes a real
  glyph from a `.notdef` box.
- `CellMetrics` changed its effective default from 12×20 to 14×21 for any
  consumer that accepts `RatatuiView`'s new default font family. No
  existing consumer (the SCTD demo included) was verified to recompute its
  terminal-grid-dependent pixel layout against the new geometry — see §5's
  Coupled item.
- `advance_blink_phase()` has zero real callers as of this writing; it is
  exercised only by its own unit tests, never by an integration path.
- No STM32H747I-DISCO hardware verification was performed for this
  initiative — host-side `cargo test`/`build`/`clippy` and an embedded
  `thumbv7em-none-eabihf` `cargo check` (compile-only) are the full extent
  of verification. Flash cost (166,088 bytes) was measured in isolation,
  not checked against the target's actual remaining flash headroom.

## §2 Divergence log

### D1 — Packed-font asset placement (caught pre-implementation)

- **Assumption:** RATATUI-00-CONCEPTS.md's first draft (§5) placed packed
  font outputs under the outer `rlvgl` repo's
  `examples/stm32h747i-disco/assets/fonts/`, mirroring the placement
  pattern of that example's own (unrelated) AA font assets.
- **Symptom:** none manifested in code — caught during a self-review pass
  before any implementation commit. Had it shipped, `ratatui-rlvgl` would
  fail `include_bytes!` resolution in a checkout of `SoftOboros/ratatui`
  alone, contradicting SCTD-04 §12's independent-build gate.
- **Root cause:** the placement was pattern-matched from a sibling
  consumer (the DISCO example) that does not share `ratatui-rlvgl`'s
  standalone-buildability constraint. The two crates' asset-locality
  requirements are different even though both consume `PackedFont`.
- **Detection gap:** no automated check would have caught this short of an
  actual fresh, superproject-patch-free clone-and-build of
  `SoftOboros/ratatui`. Verification in this initiative used git worktrees
  physically outside the parent checkout (`/tmp/rlvgl-wt-*`) as a proxy —
  a reasonably strong signal, since `include_bytes!` paths did resolve
  correctly there, but not a byte-for-byte fresh-clone test. The §12
  acceptance line "independent Ratatui-repo build (no rlvgl-superproject
  patch)" was satisfied in spirit, not to the letter.

### D2 — `isolation: "worktree"` passed alongside a manual worktree dispatch

- **Assumption:** passing the harness's `isolation: "worktree"` parameter
  in addition to a manually pre-created native worktree (with an explicit
  mandatory-`cd` instruction in the prompt) would be redundant but
  harmless — "belt and suspenders."
- **Symptom:** none directly observed; the first RATATUI-00a agent's
  deliverable landed correctly at the intended path. This appears to be
  agent robustness against a confusing instruction, not confirmation the
  combination is safe.
- **Root cause:** the orchestrating session already held the relevant
  memory (`feedback_cd_into_subrepo_before_worktree_dispatch`, describing
  this exact anti-pattern from an earlier, unrelated initiative) in
  context, but did not actively cross-check the memory against the tool
  call being constructed at dispatch time. Memory presence in context did
  not translate into active application at the moment of use.
- **Detection gap:** purely a self-discipline gap; no tooling flags
  "prompt names a manual worktree path AND `isolation` param is set" as a
  contradiction. Caught by re-reading the just-issued tool call
  immediately after dispatch, before the agent had done any real work —
  cheap to catch, but only because it was caught at all. The memory file
  was amended same-day with this recurrence (see §7).

### D3 — Cell geometry didn't fit the anticipated 12×20 grid

- **Assumption:** RATATUI-00-CONCEPTS.md §6.1 asked for the packed family
  to target `CellMetrics::font_6x10()`'s existing 12×20 geometry if
  achievable, with a new-constructor escape hatch explicitly permitted if
  not.
- **Symptom:** DejaVu Sans Mono's natural proportions do not round onto
  12×20 at any integer point size; the implementing agent discovered this
  empirically and used the escape hatch (14×21, baseline 16, at 17pt).
- **Root cause:** the spec's geometry target was aspirational — no
  font was test-packed before ratification to confirm 12×20 was reachable.
- **Detection gap:** none, really — the spec correctly anticipated this
  exact failure mode and pre-authorized the fallback, so the divergence
  cost nothing in practice. Recorded here because the pattern (a
  quantitative spec claim resolved empirically during implementation
  rather than validated before ratification) is worth naming for future
  specs — see §4 M3.

### D4 — Uneven glyph coverage across font-style variants

- **Assumption:** the curated status-symbol codepoints (specifically ✓/✗)
  were assumed to have real glyphs across all four DejaVu Sans Mono style
  variants, since they're present in the regular weight.
- **Symptom:** `DejaVuSansMono-Oblique.ttf`/`-BoldOblique.ttf` lack real
  glyphs for U+2713/U+2717; `fontdue` silently rasterizes each font's
  `.notdef` fallback shape instead.
- **Root cause:** font coverage was assumed uniform across style variants
  of "the same" font family; it isn't, at least for this Dingbats-block
  pair in this particular font family.
- **Detection gap:** the implementing agent's own regression test
  (`curated_sample_codepoints_resolve_on_every_variant`) asserts
  `font.glyph(ch).is_some()`, which is true for a `.notdef` fallback glyph
  too — `PackedFont::glyph()` doesn't distinguish "real glyph" from
  "font's placeholder box." No test in this initiative asserts glyph
  *content* (e.g. comparing the rasterized bitmap against a known-blank or
  known-box pattern), so this class of gap would recur for any future
  codepoint addition without a content-level check — see §4 M4 and §6 FC2.

## §3 Refactor points

### R1 — Font-family selection shape

- **Trigger:** RATATUI-00-CONCEPTS.md §6.2 left the exact selection-slot
  shape abstract ("a `WidgetFont`-shaped selection slot (or four, per
  §7)"), and a concrete shape was needed before dispatching the
  implementation agent.
- **Alternatives:** (a) four independent FONT-00 `WidgetFont` slots on
  `RatatuiView`; (b) one `RatatuiTerminalFont` enum bundling all four
  variants plus the `Bitmap6x10` escape hatch as a single selectable unit.
- **Selection rationale:** (b). Bold/italic selection is inherently one
  choice among four mutually exclusive options per cell; an enum models
  that exhaustively. Four independent slots would permit meaningless
  partial-assignment states (e.g. bold font set, oblique font unset) with
  no sensible interpretation in this context.
- **Cost of switch:** none paid — decided once, before any implementation
  code existed, while authoring the wiring-agent's dispatch prompt.

### R2 — Fixed-advance glyph drawing primitive

- **Trigger:** `PackedFont::draw_str`'s convenience method accumulates the
  font's natural `advance_fp16` per glyph, which is incompatible with
  RATATUI-00 §6's fixed-per-cell-advance requirement.
- **Alternatives:** (a) reimplement the coverage pipeline manually via
  `FontMetrics::shape`/`glyph_coverage_row`/`Renderer::blend_row`, as
  sketched (as a fallback option) in the wiring-agent dispatch prompt; (b)
  use `Renderer::draw_glyph`, a defaulted trait method in `rlvgl-core`
  that takes an explicit per-call pixel origin, sidestepping natural
  advance accumulation entirely.
- **Selection rationale:** (b), discovered by the implementing agent by
  inspecting the actual `rlvgl-core = "0.2.4"` dependency rather than
  assuming the outer repo's in-tree `core/` matched byte-for-byte. Reuses
  tested infrastructure; avoids reimplementing coverage math this
  initiative didn't need to own.
- **Cost of switch:** none — resolved once, correctly, on first
  implementation attempt; the dispatch prompt deliberately left this
  choice open rather than mandating (a).

## §4 Mitigation patterns

- **M1 (asset locality).** When adding an asset-bearing feature to a crate
  that must build standalone outside its usual superproject, verify the
  asset path resolves *inside that crate's own directory tree* — do not
  pattern-match placement from a sibling consumer unless it shares the
  same standalone-build constraint.
- **M2 (worktree/isolation exclusivity).** When a dispatch prompt manually
  specifies a pre-created worktree `cd` target, treat that as mutually
  exclusive with the `isolation` parameter on the same `Agent` call —
  passing both is a bug to catch before dispatch, not a defensible
  redundancy. (Now encoded in
  `feedback_cd_into_subrepo_before_worktree_dispatch`.)
- **M3 (empirical geometry before ratification).** When a spec pins a
  quantitative sizing/geometry target ahead of implementation, either
  spike it empirically first, or — as RATATUI-00 §6.1 correctly did —
  explicitly frame the target as provisional with a named, pre-authorized
  fallback, so discovering it's unreachable is a planned branch rather
  than a surprise requiring a spec amendment.
- **M4 (glyph-content verification, not just presence).** When curating a
  codepoint set across multiple font-style variants, verify real glyph
  *content* per variant (e.g. bitmap non-emptiness or divergence from the
  font's own `.notdef` shape), not just that a lookup returns `Some`.
  Style variants of "the same" font family can have divergent Unicode
  coverage.
- **M5 (independent re-verification before cherry-pick — validated, keep
  doing this).** Re-run a background agent's reported verification
  commands yourself and read the actual diff before cherry-picking; don't
  trust a self-reported summary alone. Applied successfully twice in this
  initiative (both RATATUI-00a and RATATUI-00a-wiring/00b/00c) — caught
  nothing wrong either time, but the practice is what makes that a
  meaningful finding rather than an assumption.
- **M6 (confirm staged diff before a cross-session-shared-repo commit —
  validated, keep doing this).** Before committing a gitlink advance (or
  any commit) in a repo another session might be concurrently modifying,
  run `git status`/`git diff --cached --stat` to confirm only the intended
  change is staged. Caught unrelated concurrent `streamz`/`schematic` work
  mid-initiative here; `git add <specific-path>` plus this check meant it
  was never at risk, but the check is what turned "probably fine" into
  "confirmed fine."

## §5 Deferred work reclassification

- **Safe** (orthogonal, no core-invariant impact):
  - Braille (U+2800–U+28FF) exclusion — named non-goal, reopen-gated on a
    named consumer per RATATUI-00-CONCEPTS.md §6.3/§9. Nothing else in
    this initiative depends on it.
  - Scrollback/`Viewport::Inline` non-goal — permanent by design per §9;
    not a deferral in the "will revisit" sense.
- **Coupled** (affects assumptions; must be revisited with context):
  - **The SCTD demo's hero-popup pixel layout.** SCTD-04's own §15 history
    hand-tuned title-bar insets, close-button hit regions, and the 63×17
    terminal grid against `font_6x10()`'s 12×20 geometry on real 800×480
    hardware. `RatatuiView` now defaults to `packed_terminal()`'s 14×21
    geometry. The 39 existing SCTD-demo host tests pass unchanged, which
    proves the code compiles and doesn't panic against whatever geometry
    it's actually resolving — it does **not** prove the popup's tuned
    pixel layout is still visually correct. This was not verified in
    either direction (dynamic recompute vs. explicit `Bitmap6x10` opt-in)
    during this initiative. See §6 FC1.
  - **The two `.notdef` glyphs (D4).** Dormant — no current consumer
    renders a BOLD+ITALIC or ITALIC-styled ✓/✗, so the gap has no observed
    effect yet. Revisit if/when a consumer does; see §6 FC2.
- **Abandoned:** none. All four sub-phases scoped at ratification
  (RATATUI-00a, 00a-wiring, 00b, 00c, 00d) landed; nothing was cut.

## §6 Forward constraints

- **FC1.** Before any consumer ships the new default `Packed` font family
  to a real display, its terminal-grid-dependent pixel math (popup insets,
  hit regions, title-bar layout — anything tuned against the old 12×20/
  63×17 geometry per SCTD-04 §15) MUST be re-verified against the new
  14×21/`packed_terminal()` geometry, either by confirming the layout
  recomputes from `CellMetrics` dynamically, or by explicitly opting into
  `RatatuiTerminalFont::Bitmap6x10` to preserve the old tuned geometry
  unchanged. Passing host tests are not sufficient evidence of visual
  correctness on hardware for this specific risk.
- **FC2.** Any future addition to the RATATUI-00 §6.3 curated codepoint
  set MUST verify real (non-`.notdef`) glyph *content* across all four
  style variants before ratification — presence-only checks
  (`glyph(ch).is_some()`) are not sufficient, per D4/M4.
- **FC3.** RATATUI-00c's blink phase has no real caller as of this
  retrospective. Before describing blink as user-facing-complete (as
  opposed to library-complete), wire at least one consumer's tick source
  to `advance_blink_phase()` and verify the suppress/redraw cycle
  end-to-end in the host simulator. Hardware verification is not required
  for this specific check.
- **FC4.** If Braille support is requested in the future, follow the
  reopen convention already named in RATATUI-00-CONCEPTS.md §6.3/§9
  (named consumer + §15 amendment) rather than adding it silently, and
  repeat the flash-cost-before-ratifying discipline §6.3 already
  established.

## §7 Provenance hooks

- Spec: [RATATUI-00-CONCEPTS.md](RATATUI-00-CONCEPTS.md), full §15 change
  log (DRAFT → RATIFIED → CORRECTION → RATATUI-00a IMPLEMENTED →
  RATATUI-00a-wiring/00b/00c IMPLEMENTED, all 2026-07-17).
- Companion amendment:
  [SCTD-04-RATATUI-RLVGL-INTEGRATION.md](SCTD-04-RATATUI-RLVGL-INTEGRATION.md)
  §7/§15, dated 2026-07-17, citing RATATUI-00 §8.
- Commits — `SoftOboros/ratatui` `dev/ratatui-rlvgl-backend`: `9cc8141c`
  (00d), `d9380a36` (00a), `adc75755` (00a-wiring+00b+00c). `rlvgl`
  `v0.2.6`: `1a3827a7` (ratify), `1399e4a0` (asset-locality correction,
  D1), `cdc0c59e` (gitlink advance to `adc75755`). `softoboros`
  `webslinger`: `8aa8d2b73` (ratify + `.gitmodules` fix), `665ce3497`
  (gitlink advance).
- Memory: `feedback_cd_into_subrepo_before_worktree_dispatch` (amended
  2026-07-17 with the D2 recurrence note, before this retrospective was
  drafted).

## §8 Change log

- 2026-07-18 — Drafted, covering RATATUI-00's single-day draft-to-landed
  arc (2026-07-17).
