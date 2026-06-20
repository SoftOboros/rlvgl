<!--
BEETLE-IDF-02-TOUCH.md - Milestone M3: FT5x06 capacitive touch read,
180-degree axis flip, and release debounce into one PressRelease per tap.
-->

**[← BEETLE-IDF-01](BEETLE-IDF-01-RENDER-BRIDGE.md) · [Index](README.md) · [BEETLE-IDF-03 →](BEETLE-IDF-03-DISCO-DEMO.md)**

# BEETLE-IDF-02 — FT5x06 Capacitive Touch: Read, Flip, Debounce

> **Status:** Shipped; HIL-verified 2026-06-15. Retroactive record of
> milestone **M3**. Vocabulary is owned by
> [BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md).

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| [`main/dfr0550_idf_compare.c`](../../examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c) | C host: FT5x06 register read, axis flip, per-frame touch sample | `(idf_compare.c:NN)` |
| [`components/rlvgl_app/rust/src/lib.rs`](../../examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs) | Rust payload: release debounce, touch→`PressRelease` conversion | `(rlvgl_app/lib.rs:NN)` |
| FocalTech FT5x06/FT6x36 register map | `TD_STATUS`, point byte layout, event flag | `(ft5x06)` |
| [BEETLE-00 §1](../beetle-esp32p4/BEETLE-00-CONCEPTS.md) | touch device identity (FT5x06 @ 0x38, SCL=GPIO8/SDA=GPIO7) | `(BEETLE-00 §1)` |

## §1 Purpose

Make the disco-demo UI interactive: read the FT5x06 capacitive touch
controller on the shared I2C bus, map its coordinates into screen space,
and convert the per-frame contact sample into exactly one rlvgl
`PressRelease` per physical tap.

## §2 Problem statement

1. **Mounting-rotation mismatch.** The panel is mounted point-reflected
   180° relative to the touch panel's native origin: a centre tap lands at
   centre, but motion runs *opposite* on both axes. Diagnosed on HIL —
   "centre hits, but the point moves in the opposite direction from
   centre." Both axes MUST be flipped into screen space.
2. **Mid-press jitter.** Capacitive panels routinely drop a contact for a
   single frame during a hold. A raw edge-per-frame would fragment one
   physical tap into several events. Release MUST be debounced. See
   INV-BEETLE-IDF-5.

## §3 Glossary

**Release debounce** and the **render entry** are defined in
[BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md) and used here without
modification. M3 is the chapter that *owns* the release-debounce concept
(INV-BEETLE-IDF-5) and that grew the render entry's touch parameters
(INV-BEETLE-IDF-1). This chapter adds no new vocabulary.

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| FT5x06 register read + parse | C host (`touch_read`, `idf_compare.c:151`) |
| 180° axis flip | C host (`touch_read`, `idf_compare.c:175-176`) |
| Render entry touch parameters | INV-BEETLE-IDF-1 (frozen, concepts gate) |
| Touch sample → `PressRelease` | Rust payload (`rlvgl_app_render`, `rlvgl_app/lib.rs:330-343`) |
| Release debounce rule | INV-BEETLE-IDF-5 (frozen) |

## §5 Authority relationship matrix

| External authority | Concept | Relationship | Mutation rights | Divergence policy |
|---|---|---|---|---|
| FT5x06 register map | `TD_STATUS` + point layout | mirror | none | byte parse matches the part datasheet |
| ESP-IDF `i2c_master` | bus transaction | consume | none — fixed dependency | pin IDF v5.3.5 |
| `rlvgl-core::Event` | `PressRelease` | mirror | none — owned by rlvgl-core | upstream changes break this consumer at rebuild |

## §6 Frozen enums

None. The FT5x06 event flag (0=down, 1=up, 2=contact, 3=no-event) is a
hardware constant, not an rlvgl enum.

## §7 Frozen timing & topology

- **Touch device:** FT5x06/FT6x36 @ I2C `0x38`, sharing the bus with the
  `0x45` bridge (SCL=GPIO8, SDA=GPIO7). Read each frame, point 1 only.
- **Register read:** 5 bytes from `0x02` (`TD_STATUS`):
  - byte0 low nibble = number of points;
  - byte1 (XH): bits[7:6] = event (0=down, 1=up, 2=contact, 3=none),
    low nibble = x_hi;
  - byte2 (XL) = x_lo;
  - byte3 (YH) low nibble = y_hi; byte4 (YL) = y_lo.
  (`idf_compare.c:153-168`)
- **Axis flip (both axes):** `x = (DFR0550_H_RES-1) - raw_x = 799 - raw`;
  `y = (DFR0550_V_RES-1) - raw_y = 479 - raw` (`idf_compare.c:175-176`).
- **Release debounce:** `RELEASE_DEBOUNCE_FRAMES = 3` consecutive
  no-touch frames confirm a lift — ≈100 ms at the C loop's ~30 Hz
  (`rlvgl_app/lib.rs:231`).

## §9 Frozen invariants

This chapter is the implementation home of two concepts-gate freezes:

- **INV-BEETLE-IDF-1** (render entry signature) — M3 is where the entry
  grew `touch_x, touch_y, touch_active`; the 6-argument form is now
  frozen (`rlvgl_app/lib.rs:300-307`). The C host passes the latest touch
  sample through `render_rlvgl` (`idf_compare.c:291-293, 384`).
- **INV-BEETLE-IDF-5** (release-debounced single tap) — a finger lift is
  confirmed only after `RELEASE_DEBOUNCE_FRAMES` consecutive no-touch
  frames; exactly one `PressRelease` is dispatched per physical tap, at
  the *last in-contact* coordinate (the lift frame itself carries no
  coordinate). The debounce state lives in `AppState`
  (`in_contact`, `idle_frames`, `last_x`, `last_y`,
  `rlvgl_app/lib.rs:233-244`) and is driven at
  `(rlvgl_app/lib.rs:330-343)`.

## §10 Reconciliation vs adjacent repo primitives

- **vs. the C host's serial trace.** The C refill loop also emits its own
  rising/falling-edge and throttled-move serial traces for position
  validation (`idf_compare.c:370-382`). That trace is diagnostic only; the
  authoritative tap dispatch is the Rust-side debounce, not the C trace.
- **vs. STM32/BBB input adapters.** Those platforms feed `PressRelease`
  through an `rlvgl-platform` `Input` impl. The IDF-hybrid has no `Input`
  adapter; the C host samples touch and the payload converts edges
  in-line. The user-facing event (`Event::PressRelease`) is identical.

## §11 Non-goals

- Multi-touch / gestures. Point 1 only; one tap → one `PressRelease`.
- Drag/move dispatch. The demo's `ActionHotspot`s consume taps, not drags
  (the slider in [BEETLE-IDF-04](BEETLE-IDF-04-BACKLIGHT.md) is
  tap-to-position, not drag).
- An `rlvgl-platform` `Input` adapter for FT5x06 (deferred).

## §12 Acceptance checklist

- [x] (a) The FT5x06 is read each frame on the shared I2C bus; point 1
      parses from `TD_STATUS` (`idf_compare.c:151-178`).
- [x] (b) Both axes are flipped into screen space; a tap "follows my
      finger" rather than mirroring (HIL 2026-06-15).
- [x] (c) The render entry carries `touch_x/touch_y/touch_active`;
      signature frozen (INV-BEETLE-IDF-1).
- [x] (d) Release debounce collapses single-frame dropouts; one physical
      tap → exactly one `PressRelease` at the last in-contact point
      (INV-BEETLE-IDF-5, HIL 2026-06-15).
- [x] (e) A magenta feedback dot marks the live contact point each frame
      (`rlvgl_app/lib.rs:375-385`).

## §13 Files cited

- `examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c` (`touch_read`, refill loop)
- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs` (`AppState`, `rlvgl_app_render`)

## §14 Unblocks

- [BEETLE-IDF-03](BEETLE-IDF-03-DISCO-DEMO.md) (M4) — taps now reach the
  disco-demo `ActionHotspot`s, making the widget tree interactive.
- [BEETLE-IDF-04](BEETLE-IDF-04-BACKLIGHT.md) (M5) — tap-to-position on
  the backlight slider depends on this single-tap dispatch.

## §15 Change log

- **2026-06-19** (ratified retroactively) — documents work that shipped on
  the v0.2.4 branch (BEETLE M1/M3/M4/M5), merged to main in #216 /
  `5187ce0`.

---

**[← BEETLE-IDF-01](BEETLE-IDF-01-RENDER-BRIDGE.md) · [Index](README.md) · [BEETLE-IDF-03 →](BEETLE-IDF-03-DISCO-DEMO.md)**
