<!--
BEETLE-IDF-05-STAR-CRAWL.md - Software Star-Wars crawl effect on the
ESP-IDF-hybrid P4 payload, driven by a drained StartEffect(StarCrawl).
-->

**[← BEETLE-IDF-04](BEETLE-IDF-04-BACKLIGHT.md) · [Index](README.md)**

# BEETLE-IDF-05 — Software Star Crawl

> **Status:** Implemented; HIL-verified 2026-06-19. Depends on chapters
> 01–04 (render bridge, touch, disco-demo mount, command drain) — all
> shipped.

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| [BEETLE-IDF-00](BEETLE-IDF-00-CONCEPTS.md) | Track vocabulary, INV-BEETLE-IDF-1..5 | `(BEETLE-IDF-00 §N)` |
| [`disco-demo`](../../examples/apps/disco-demo/src/lib.rs) | `DiscoEffect::StarCrawl`, `DiscoCommand::StartEffect` | `(disco-demo/src/lib.rs:NN)` |
| [`rlvgl_app/lib.rs`](../../examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs) | Render entry, command drain, `Rgb888Renderer` | `(rlvgl_app/lib.rs:NN)` |
| `rlvgl-core` `bitmap_font::FONT_6X10` | Glyph bitmaps for scaled text | `(core/src/bitmap_font.rs)` |
| [`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs) | STM32 crawl — reconciliation only, **not reused** | `(stm32 star_crawl.rs)` |

## §1 Purpose

Run a recognizable Star-Wars-style scrolling-text crawl over a starfield
on the DFR0550-V2 panel when the user taps **Info → Star Crawl**, and
dismiss it on the next tap. This is the hybrid track's first piece of
full-screen dynamic content and closes the concepts-gate acceptance item
[BEETLE-IDF-00 §12 (e)](BEETLE-IDF-00-CONCEPTS.md).

The shared controller already does its half: tapping the Star Crawl info
item with `effects: true` queues exactly one
`DiscoCommand::StartEffect(DiscoEffect::StarCrawl)`
(`disco-demo/src/lib.rs:731`). The runtime adapter owns the rest — the
controller emits **no** automatic `StopEffect`; lifecycle is the
adapter's. Today the P4 payload drains and drops that command
(BEETLE-IDF-03 §2). This chapter makes the payload *run* the effect.

## §2 Problem statement

1. **The STM32 crawl is not portable.** `star_crawl.rs` on the DISCO is
   coupled to DMA2D, fixed SDRAM scratch addresses (`0xD1xx_xxxx`), a D2
   SRAM A8 buffer, DWT-cycle admission gating, and ARGB8888. None of that
   exists on the P4 software path, which has only `Rgb888Renderer`'s
   `fill_rect` into a PSRAM B,G,R framebuffer. A new, self-contained
   software crawl is required.
2. **One frame, ~33 ms, no GPU.** The crawl renders entirely on the HP
   CPU inside the render entry's per-frame slice
   ([BEETLE-IDF-00 §7](BEETLE-IDF-00-CONCEPTS.md)). It MUST stay cheap
   enough to keep the refill loop above the 30 Hz bridge-desync floor.
3. **No `Math::random` / `Date::now`.** The payload is no_std; the
   starfield MUST be generated deterministically (seeded xorshift), not
   from a runtime RNG.
4. **Effect vs. widget tree.** While the crawl is active it owns the
   whole framebuffer; the widget tree MUST NOT also draw, and touch MUST
   dismiss the crawl rather than dispatch to hotspots.

## §3 Canonical glossary

- **`StarCrawl` (P4)** — the software effect struct in the new
  `star_crawl` module of the Rust payload. Holds activation state, a
  scroll phase, and a deterministic starfield. **Owned by this chapter;
  code mirrors after ratification.** Distinct from the DISCO
  `StarCrawl` in `(stm32 star_crawl.rs)`.
- **Crawl script** — the `&'static [&'static str]` of text lines the
  crawl scrolls. **Owned by this chapter** (embedded in the P4 payload;
  not shared with the DISCO `README_CRAWL`).
- **Scroll phase** — the monotonically increasing `f32` that advances
  each frame and drives every line's perspective depth. **Owned by this
  chapter.**
- **`DiscoEffect::StarCrawl` / `StartEffect`** — the shared command
  surface that triggers the effect. **Defined in
  `(disco-demo/src/lib.rs)`; used without modification.**

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| Effect trigger command | `disco-demo` (`StartEffect(StarCrawl)`) — code is canonical |
| Effect lifecycle (start/run/dismiss) | This chapter — adapter-owned, §9 |
| Crawl renderer (starfield + perspective text) | This chapter (`star_crawl` module) |
| Glyph bitmaps | `rlvgl-core` `FONT_6X10` — code is canonical |
| Frame clear / writeback | C host + `Rgb888Renderer` (INV-BEETLE-IDF-3) |

## §6 Frozen enums

No new enums. `DiscoEffect` is **frozen under Standards Action** in
disco-demo; this chapter adds no variant and reuses the existing
`StarCrawl`.

## §7 Frozen timing & topology

- **Scroll rate:** `PHASE_PER_FRAME = 0.012` phase-units/frame. At ~30 Hz
  one full script traversal takes on the order of ~30–60 s. Tunable
  within ±50% without a §15 amendment (it is a feel parameter, not a
  contract).
- **Perspective:** a line's depth `d = 1.0 + age * DEPTH_PER_AGE`, where
  `age = phase - line_index * LINE_SPACING`. On-screen top
  `y = HORIZON_Y + (BASE_Y - HORIZON_Y) / d`; integer pixel scale
  `s = max(1, round(BASE_SCALE / d))`. A line is drawn only while
  `0 < age` and `d ≤ D_FAR`. Lines enter large at `BASE_Y` (near the
  bottom) and shrink toward `HORIZON_Y`.
- **Starfield:** `STAR_COUNT = 96` stars, positions and brightness from a
  fixed-seed xorshift filled once at `start()`. Stars drift upward slowly
  (`phase`-derived) for parallax and wrap modulo screen height.
- **Resolution-relative:** all of `HORIZON_Y`, `BASE_Y` derive from the
  render entry's `width`/`height`, so the effect is correct at 800×480
  and any future panel size.

## §9 Frozen invariants

### INV-BEETLE-IDF-5-1 — Adapter owns the crawl lifecycle

The crawl MUST activate on a drained
`StartEffect(DiscoEffect::StarCrawl)`, run autonomously across frames,
and deactivate on **either** (a) a touch-down while active, **or** (b)
the scroll phase passing the end of the script. The controller emits no
`StopEffect` for the crawl; the adapter MUST NOT depend on one.

**Registration policy:** **Specification Required**.

### INV-BEETLE-IDF-5-2 — Active crawl suppresses the widget tree and touch dispatch

While the crawl is active, the render entry MUST NOT draw the widget tree
and MUST NOT dispatch touch as `PressRelease` to the controller. A
touch-down is consumed as a dismiss (INV-BEETLE-IDF-5-1). The controller
is still `tick()`ed and its command queue still drained each frame so it
does not stall or grow unbounded.

**Registration policy:** **Specification Required**.

### INV-BEETLE-IDF-5-3 — Deterministic starfield

The starfield MUST be generated from a fixed seed (no runtime RNG, no
clock). Re-running the effect produces the same field. **Specification
Required.**

### INV-BEETLE-IDF-5-4 — Crawl honors the frame contract

The crawl draws through `Rgb888Renderer` (B,G,R, INV-BEETLE-IDF-4) and
fully paints the framebuffer each active frame (it clears to space-black
itself, satisfying INV-BEETLE-IDF-3 for the frames it owns). It adds no
new C-ABI surface — INV-BEETLE-IDF-1 is unchanged. **Standards Action**
(touches the frame contract).

## §10 Reconciliation vs adjacent repo primitives

- **vs. DISCO `star_crawl.rs`.** Same user-facing contract
  (`DiscoEffect::StarCrawl`), entirely different implementation. The
  DISCO version is a non-blocking DMA2D state machine with FIR text
  resampling and an A8 blend; the P4 version is a straight-line software
  raster (perspective via integer glyph block-scaling). Neither imports
  the other. A future consolidation MAY extract a platform-agnostic
  software crawl that both the BBB Linux prong and this track share; that
  is **out of scope** here (noted, not built — resurrection guard for
  future agents tempted to unify prematurely).
- **vs. `disco-demo`.** Pure consumer. No change to the shared crate;
  the crawl lives entirely in the P4 payload. This is deliberate: the
  effect renderer is platform-specific (software vs. DMA2D), so it does
  not belong in the shared, platform-agnostic controller.
- **vs. the crawl script.** The DISCO pulls `README_CRAWL` from its own
  module; the P4 embeds a short self-contained script. Sharing crawl
  *text* is a cosmetic future nicety, not a contract.

## §11 Non-goals

- **FIR-smoothed / anti-aliased glyphs.** Integer block-scaled
  `FONT_6X10` is the v1 fidelity bar. AA text is a future nicety.
- **Splash graphic / logo bitmaps.** The DISCO crawl blits a 384×384
  splash + a logo; the P4 v1 is text + stars only.
- **Sharing the renderer with STM32 / BBB.** §10.
- **Audio-reactive / spectrum effects.** Other `DiscoEffect` variants
  remain drained-and-dropped on this hybrid.

## §12 Acceptance checklist

A conforming BEETLE-IDF-05 implementation MUST:

- [x] (a) Activate the crawl on a drained `StartEffect(StarCrawl)`
      (INV-BEETLE-IDF-5-1).
- [x] (b) Render a starfield + perspective scrolling text through
      `Rgb888Renderer`, advancing each frame (INV-BEETLE-IDF-5-4).
- [x] (c) Suppress the widget tree and touch dispatch while active;
      dismiss on touch-down (INV-BEETLE-IDF-5-2).
- [x] (d) Deactivate at end-of-script and return to the widget tree
      (INV-BEETLE-IDF-5-1).
- [x] (e) Generate the starfield deterministically (INV-BEETLE-IDF-5-3).
- [x] (f) Keep the build green: the staticlib compiles for
      `riscv32imafc-unknown-none-elf` and host unit tests (the crawl's
      pure layout math) pass under `cargo test`.
- [x] (g) **HIL:** tapping Info → Star Crawl runs the crawl; a tap
      dismisses it; the home screen returns intact; the refill loop stays
      above 30 Hz (no bridge desync to white).

## §13 Files cited

- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs`
  (render entry, drain, renderer)
- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/star_crawl.rs`
  (new — this chapter)
- `examples/apps/disco-demo/src/lib.rs` (`StartEffect(StarCrawl)` emit)
- `core/src/bitmap_font.rs` (`FONT_6X10`)
- `examples/stm32h747i-disco/src/star_crawl.rs` (reconciliation only)

## §14 Unblocks

- Closes [BEETLE-IDF-00 §12 (e)](BEETLE-IDF-00-CONCEPTS.md), the last
  open gate-level acceptance item.
- Gives the hybrid track a dynamic-content reference for any future
  full-screen effect (file browser animation, audio scope) that wants to
  own the framebuffer the same way.

## §15 Change log

- **2026-06-19** (ratified) — Spec authored ahead of implementation
  (spec-before-code). Frozen the lifecycle (INV-BEETLE-IDF-5-1),
  widget/touch suppression (INV-BEETLE-IDF-5-2), deterministic starfield
  (INV-BEETLE-IDF-5-3), and frame-contract conformance
  (INV-BEETLE-IDF-5-4). Perspective and timing constants set in §7 as
  tunable feel parameters. Implementation lands in the `star_crawl`
  module of the P4 payload with host-testable layout math.
- **2026-06-19** (HIL-verified) — Flashed to the DFR1237 + DFR0550-V2
  bench. Tapping Info → Star Crawl runs the starfield + perspective
  scrolling text; a tap dismisses it and the home screen returns intact;
  the refill loop stays above the 30 Hz desync floor (no white-out).
  Acceptance items (a)–(g) closed.

---

**[← BEETLE-IDF-04](BEETLE-IDF-04-BACKLIGHT.md)** · **[Index](README.md)**
