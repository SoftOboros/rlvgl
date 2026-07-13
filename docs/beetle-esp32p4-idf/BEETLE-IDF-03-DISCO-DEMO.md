<!--
BEETLE-IDF-03-DISCO-DEMO.md - Milestone M4: mount the shared disco-demo
widget tree (no fork), per-frame clear, alpha-aware draw_pixels override.
-->

**[← BEETLE-IDF-02](BEETLE-IDF-02-TOUCH.md) · [Index](README.md) · [BEETLE-IDF-04 →](BEETLE-IDF-04-BACKLIGHT.md)**

# BEETLE-IDF-03 — Mounting the Shared disco-demo Widget Tree

> **Status:** Shipped; HIL-verified 2026-06-15. Retroactive record of
> milestone **M4**. Vocabulary is owned by
> [BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md).

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| [`components/rlvgl_app/rust/src/lib.rs`](../../examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs) | Rust payload: controller mount, per-frame clear, `draw_pixels` override, command drain | `(rlvgl_app/lib.rs:NN)` |
| [`examples/apps/disco-demo/src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) | shared `DiscoController` / `DiscoCapabilities` / `DiscoCommand` | `(disco-demo/lib.rs:NN)` |
| `rlvgl-core::Renderer` / `Event` | `draw_pixels` default, `PressRelease` dispatch | `(core/...)` |
| [BEETLE-IDF-00 §6 INV-BEETLE-IDF-3](BEETLE-IDF-00-CONCEPTS.md) | per-frame clear + writeback | `(BEETLE-IDF-00 §6)` |

## §1 Purpose

Mount the *shared* `rlvgl-app-disco-demo` widget tree — the same crate
running on STM32H747I-DISCO and the BBB Linux prong — on the IDF-hybrid
renderer, making the FireBeetle the third platform variant of the payload
and the first interactive one. M4 wires touch (M3) into a real widget
tree.

## §2 Problem statement

The disco-demo **root container is deliberately transparent** (alpha=0):
on STM32 it composites over a desktop/splash layer that paints the
background. **On the P4 there is no desktop layer.** Without an explicit
full-frame clear every frame, the transparent root never erases the prior
frame, so:

- stale wing pixels accumulate — a wing looked like it "opened but did
  not close";
- magenta feedback dots piled up across the frame.

The root cause was **not** a double-toggle in the controller; it was the
**missing per-frame clear**. A second problem surfaced when icons drew:
the `Renderer` trait's default `draw_pixels` routes each pixel through
`fill_rect` (an opaque `put`), painting the RLE icons' fully-transparent
pixels as solid black boxes.

## §3 Glossary

`DiscoController`, `DiscoCommand`, `DiscoEffect`, `DiscoCapabilities`,
**render entry**, and `Rgb888Renderer` are all defined in
[BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md) and used without
modification. This track is a *consumer* of the disco-demo types and MUST
NOT fork them. This chapter adds no new vocabulary.

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| Widget tree / controller / capabilities | `examples/apps/disco-demo/` (shared; code is canonical) |
| Controller mount + lifecycle on P4 | Rust payload (`AppState`, `rlvgl_app/lib.rs:233-273`) |
| Per-frame clear | INV-BEETLE-IDF-3 (frozen); impl `rlvgl_app/lib.rs:357-371` |
| Alpha-aware `draw_pixels` | Rust payload override (`rlvgl_app/lib.rs:202-211`) |
| Command drain policy | Rust payload (`rlvgl_app/lib.rs:349-355`) |

## §5 Authority relationship matrix

| External authority | Concept | Relationship | Mutation rights | Divergence policy |
|---|---|---|---|---|
| `examples/apps/disco-demo/` | widget tree, `DiscoCommand`/`DiscoEffect` | compose | none — shared with DISCO + BBB | upstream changes land at next rebuild |
| `rlvgl-core::Renderer::draw_pixels` | pixel-span default | override | the override is local to `Rgb888Renderer`; the trait default is unchanged | upstream trait changes break this consumer at rebuild |

## §6 Frozen enums

None. `DiscoCommand`/`DiscoEffect` are frozen in disco-demo under
Standards Action; this track adds no variants
([BEETLE-IDF-00 §6](BEETLE-IDF-00-CONCEPTS.md)).

## §7 Frozen timing & topology

- **Capabilities advertised:** `DiscoCapabilities { audio: false,
  storage: false, diagnostics: true, effects: true, pointer: true,
  platform: "ESP32-P4 ESP-IDF" }` (`rlvgl_app/lib.rs:252-259`).
- **Lifecycle:** the controller is held in a process-global
  `static mut APP: Option<AppState>` (`rlvgl_app/lib.rs:273`),
  lazy-initialized on the first render call once the FB dimensions are
  known (`rlvgl_app/lib.rs:318-320`). A plain `static` is impossible —
  the controller owns non-`Sync` `Rc<RefCell<…>>` graphs; the `static
  mut` is sound because the single FreeRTOS render task calls the entry
  serially (no concurrency, `rlvgl_app/lib.rs:218-225`).
- **Per-frame sequence** (`rlvgl_app/lib.rs:323-385`):
  1. `controller.tick()` — drives animations, live info pages, focus pulse;
  2. debounce the touch sample into a `PressRelease` (M3);
  3. drain the command queue;
  4. **clear** the whole FB to `Color(16,20,32,255)`;
  5. draw the widget tree;
  6. draw the magenta feedback dot at the live contact.

## §9 Frozen invariants

- **INV-BEETLE-IDF-3** (clear + writeback) — M4 implements the *clear*
  half: the payload MUST `fill_rect` the whole frame before drawing,
  every frame, because the root is transparent and the double buffers
  ping-pong so each must be cleared on the frame it draws
  (`rlvgl_app/lib.rs:357-371`). HIL after the fix: the settings wing
  "opens and closes now." The writeback half is the C host's
  `esp_cache_msync` (M1).

This chapter mints one local invariant:

### INV-BEETLE-IDF-3a — Alpha-aware pixel spans

`Rgb888Renderer::draw_pixels` MUST route each pixel through `blend` (not
the trait-default opaque `fill_rect` path), so per-pixel alpha and the BGR
swap (INV-BEETLE-IDF-4) are honored. This is the path `blit_image` uses to
composite the demo's RLE icons; the default path renders their transparent
pixels as solid black boxes (`rlvgl_app/lib.rs:198-211`).

**Registration policy:** **Specification Required** (local to the
renderer; the compositing requirement is fixed by the icon format).

## §10 Reconciliation vs adjacent repo primitives

- **Shared crate, no fork.** This is a pure *consumer* of
  `examples/apps/disco-demo/`; the controller, capabilities, and command
  surface are unmodified. The P4-specific code is the mount glue
  (`AppState`) and the renderer, not a forked widget tree.
- **vs. STM32 desktop layer.** The transparency assumption baked into the
  disco-demo root is correct on STM32 (desktop composites under it) and
  wrong on the bare P4 FB. INV-BEETLE-IDF-3's per-frame clear is the
  IDF-hybrid's substitute for the absent desktop layer — it does not
  change the shared crate.

## §11 Non-goals

- Running effects/status commands other than `SetBacklight`. The drain
  loop matches only `DiscoCommand::SetBacklight` (M5); every other
  command (effects, status) is **drained-and-dropped** — draining keeps
  the controller's queue bounded even though no runtime consumes them yet
  (`rlvgl_app/lib.rs:349-355`). In particular
  `StartEffect(DiscoEffect::StarCrawl)` is dropped here, which is exactly
  what [BEETLE-IDF-05](BEETLE-IDF-05-STAR-CRAWL.md) fixes.
- Audio / storage (capabilities advertise `false`).

## §12 Acceptance checklist

- [x] (a) The shared `rlvgl-app-disco-demo` crate mounts unforked, with
      the P4 capabilities (`rlvgl_app/lib.rs:247-262`).
- [x] (b) The controller persists across frames in `APP`, lazy-built on
      first render (`rlvgl_app/lib.rs:273, 318-321`).
- [x] (c) The framebuffer is cleared every frame; wings open *and* close,
      no pixel accumulation (INV-BEETLE-IDF-3, HIL 2026-06-15).
- [x] (d) `draw_pixels` blends per-pixel alpha; RLE icons composite
      without black boxes (INV-BEETLE-IDF-3a).
- [x] (e) Taps reach the demo's `ActionHotspot`s via `dispatch_event`,
      which stops at the first handler; non-`SetBacklight` commands are
      drained and dropped (`rlvgl_app/lib.rs:341, 349-355`).

## §13 Files cited

- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs`
- `examples/apps/disco-demo/src/lib.rs` (`DiscoController`, `DiscoCapabilities`)

## §14 Unblocks

- [BEETLE-IDF-04](BEETLE-IDF-04-BACKLIGHT.md) (M5) — the drained
  `SetBacklight` command becomes a live panel dim.
- [BEETLE-IDF-05](BEETLE-IDF-05-STAR-CRAWL.md) (M6) — the dropped
  `StartEffect(StarCrawl)` command gains a runtime renderer.

## §15 Change log

- **2026-06-19** (ratified retroactively) — documents work that shipped on
  the v0.2.4 branch (BEETLE M1/M3/M4/M5), merged to main in #216 /
  `5187ce0`.

---

**[← BEETLE-IDF-02](BEETLE-IDF-02-TOUCH.md) · [Index](README.md) · [BEETLE-IDF-04 →](BEETLE-IDF-04-BACKLIGHT.md)**
