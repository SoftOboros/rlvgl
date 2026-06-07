<!--
BEETLE-08-DEMO-INTEGRATION.md - rlvgl widget tree mount on the live FB.
v1 goal. Not started.
-->

**[← BEETLE-07](BEETLE-07-CACHE.md) · [Index](README.md)**

# BEETLE-08 — Disco-Demo Widget Tree on the Live Framebuffer

> **Implementation status:** Not started. v1 conformance gate.
> Depends on v0 conformance (chapters 02–07 ratified + chapter 06
> implementation landed).

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| `examples/apps/disco-demo/` | Shared widget tree, app state machine, render entry point | `(disco-demo/src/...)` |
| `rlvgl-core` | `Widget` trait, layout, event dispatch | `(core/...)` |
| `rlvgl-widgets` | Concrete widgets used by disco-demo | `(widgets/...)` |
| `rlvgl-platform` | `Display` / `Input` traits, embedded-graphics adapters | `(platform/...)` |
| BEETLE-06 | FB lifetime + provenance | `(BEETLE-06 §9)` |
| BEETLE-00 §9 INV-BEETLE-00-4 | Continuous re-fill loop | `(BEETLE-00 §9)` |

## §1 Purpose

Mount the shared disco-demo widget tree (already running on
STM32H747I-DISCO and the BBB Linux prong) on the live DPI-driven
framebuffer. v1 conformance gate.

The work is two-sided:
1. **Display adapter**: provide an `rlvgl-platform`-compatible
   `Display` impl whose `flush()` writes into the FB pointer
   returned by `DpiPanel::init` and calls `cache::writeback`
   per INV-BEETLE-00-3.
2. **Re-fill loop**: integrate the widget tree's render cycle with
   the bridge's continuous-refresh requirement per INV-BEETLE-00-4.

## §2 Problem statement

The DFR0550-V2 bridge desyncs to white if the CPU stops touching
the FB. The widget tree's render cycle is event-driven (idle when
no widget is dirty), so a naive `widget.render() → flush()` loop
will exit to idle and trigger desync.

Two compatible models:

- **Continuous full re-render.** Treat the bridge's refresh
  requirement as a hard 30 Hz floor; re-render the whole widget tree
  every frame regardless of dirty state. Wastes CPU on static
  scenes; trivial to implement.
- **Sentinel touch-up.** Render only dirty rectangles, but on every
  vsync (or every ~16 ms tick) write at least one pixel + cache-
  writeback to keep the bridge syncronized. Cheaper on static
  scenes; requires a vsync-like timer.

Decision deferred to BEETLE-08a implementation. Bench measurement
will inform: if 800×480×60 Hz RGB888 widget render is < 30% of HP
CPU at 400 MHz, prefer continuous full re-render for simplicity.

## §3 Canonical glossary

- **Display adapter** — Concrete `Display` impl bridging
  `rlvgl-platform` to the DPI FB. **Owned by BEETLE-08; analogous
  to `platform/src/display.rs` on STM32H747I-DISCO.**
- **Re-fill loop** — Top-level application loop in
  `bsp_pac_main.rs::main` (post-bring-up). Performs: poll input →
  dispatch events → widget render → flush → cache writeback →
  delay-to-frame.
- **Frame budget** — 16.67 ms at 60 Hz. The re-fill loop's combined
  cost (input poll + widget render + flush + writeback) MUST fit
  in this budget for steady-state.

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| `disco-demo` widget tree | `examples/apps/disco-demo/` (code is canonical) |
| `Display` trait | `rlvgl-platform` (code is canonical) |
| Display adapter impl | This chapter §9 INV-BEETLE-08-1 (code mirrors after BEETLE-08a) |
| Re-fill loop shape | This chapter §9 INV-BEETLE-08-2 |

## §5 Authority relationship matrix

Inherits from BEETLE-00 §5. Adds the disco-demo crate row:

| External authority | Concept | Relationship | Mutation rights | Divergence policy |
|---|---|---|---|---|
| `examples/apps/disco-demo/` | Shared widget tree | compose | none — payload is shared with DISCO + BBB | upstream changes in disco-demo land in this consumer at the next rebuild |
| `rlvgl-platform::Display` | `Display` trait | mirror | none — trait surface is owned by rlvgl-platform | upstream trait changes break this consumer at the next rebuild |

## §6 Frozen enums

None this chapter — no new enums. May introduce a `ReFillStrategy`
enum (`AlwaysFull / SentinelTouchUp`) during BEETLE-08a if both
strategies survive bench measurement.

## §7 Frozen timing & topology

*To be finalized during BEETLE-08a. Anchor points:*

- **Frame rate target:** 60 Hz nominal; 30 Hz floor (anything
  slower triggers bridge desync).
- **Widget render budget:** TBD, depends on disco-demo's actual
  cost.
- **FB writeback cadence:** every frame (continuous full re-render)
  or every vsync (sentinel touch-up).

## §8 (reserved)

## §9 Frozen invariants

### INV-BEETLE-08-1 — Display adapter wraps FB pointer

The `Display` impl MUST be parameterized by the `FrameBuffer<'static>`
returned by `DpiPanel::init`. The pointer is "binary forever" valid;
the lifetime parameter on `FrameBuffer<'p>` SHOULD elide to `'static`
in this consumer.

The adapter's `flush()` MUST write RGB888 packed pixels (matching
INV-BEETLE-06-1) and MUST call `cache::writeback(fb.ptr, len)` for
the modified range before returning.

**Registration policy:** **Standards Action**.

### INV-BEETLE-08-2 — Re-fill loop sustains ≥30 Hz cache-writeback cadence

The re-fill loop MUST issue at least one `cache::writeback` covering
≥1 byte of the FB at ≥30 Hz, irrespective of widget-render dirty
state. This MAY be:

- A whole-FB writeback after a whole-FB re-render (continuous full
  re-render strategy), or
- A 64-B (one cache line) sentinel writeback driven by a periodic
  timer (sentinel touch-up strategy).

Below 30 Hz the bridge desyncs to white per INV-BEETLE-00-4.

**Registration policy:** **Specification Required** (the strategy
choice between the two is local; the floor frequency is not).

### INV-BEETLE-08-3 — Bring-up gates demo mount

The disco-demo mount MUST run only after `run_bringup()` returns
`BringUpStatus::AllOk`. Mounting on a partial bring-up produces
silent corruption (FB writes succeed but the panel is unconfigured).

**Registration policy:** **Standards Action**.

## §10 Reconciliation vs adjacent repo primitives

The `examples/apps/disco-demo/` crate is shared with
STM32H747I-DISCO and BBB+NHD cape consumers. Its widget tree and
state machine are platform-agnostic; this chapter adds a third
`Display` adapter (alongside the existing DISCO `Ltdc`-backed
adapter and the BBB fbdev adapter).

`rlvgl-platform` and `rlvgl-core` are not modified by this chapter.
`rlvgl-widgets` likewise unchanged.

A future chapter (likely `BEETLE-TOUCH-NN` in a separate initiative)
will add the `Input` impl for FT5x06. Until then, disco-demo runs
input-less on this hardware.

## §11 Non-goals

- Input (touch / button). Future initiative.
- Audio. Out of scope per BEETLE-00 §11.
- Dynamic widget hot-reload. The widget tree is built at boot.
- Multi-window / multi-screen. Single-FB / single-screen.

## §12 Acceptance checklist

A conforming BEETLE-08 implementation MUST:

- [ ] (a) Provide a `Display` adapter wrapping the
      `FrameBuffer<'static>` from `DpiPanel::init`.
- [ ] (b) Drive a re-fill loop per INV-BEETLE-08-2.
- [ ] (c) Run only after `run_bringup() == AllOk`.
- [ ] (d) Mount the disco-demo widget tree (shared crate, no fork).
- [ ] (e) **HIL verification:** disco-demo's home screen paints,
      remains stable for ≥5 minutes without bridge desync. Star
      crawl or equivalent dynamic content optional but recommended.
- [ ] (f) **HIL verification:** frame rate sustains ≥30 Hz under
      the chosen re-fill strategy.

## §13 Files cited

- `examples/apps/disco-demo/` (entire crate; canonical payload)
- `examples/beetle-esp32p4/src/bsp_pac_main.rs:main` (post-bring-up)
- `platform/src/display.rs` (DISCO precedent for `Display` impl)
- Reference precedents on other platforms:
  - DISCO: `examples/stm32h747i-disco/src/*.rs`
  - BBB: `examples/beaglebone-black/src/bsp/*.rs`

## §14 Unblocks

- **v1 conformance** (disco-demo running on this hardware as the
  third platform variant).
- Future `BEETLE-TOUCH-NN` initiative gains a live demo to drive
  input against.

## §15 Change log

- **2026-05-28** (initial shell) — Authored as part of the BEETLE
  family setup. No implementation yet; v1 goal. §3, §7 marked
  partially open pending BEETLE-08a strategy decision. Real
  ratification with measured re-fill strategy waits for BEETLE-06a
  implementation (so disco-demo render cost can be measured against
  the live FB).

---

**[← BEETLE-07](BEETLE-07-CACHE.md)** · **[Index](README.md)**
