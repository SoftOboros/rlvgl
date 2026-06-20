<!--
BEETLE-IDF-04-BACKLIGHT.md - Milestone M5: live backlight control via the
DFR0550 bridge PWM hook plus the shared BacklightPanel slider widget.
-->

**[← BEETLE-IDF-03](BEETLE-IDF-03-DISCO-DEMO.md) · [Index](README.md) · [BEETLE-IDF-05 →](BEETLE-IDF-05-STAR-CRAWL.md)**

# BEETLE-IDF-04 — Live Backlight: Bridge PWM Hook + Shared Slider

> **Status:** Shipped; HIL-verified 2026-06-15. Retroactive record of
> milestone **M5**. Vocabulary is owned by
> [BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md).

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| [`main/dfr0550_idf_compare.c`](../../examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c) | C host: `rlvgl_host_set_backlight` → `REG_PWM` | `(idf_compare.c:NN)` |
| [`components/rlvgl_app/rust/src/lib.rs`](../../examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs) | Rust payload: `SetBacklight` drain → host hook | `(rlvgl_app/lib.rs:NN)` |
| [`examples/apps/disco-demo/src/backlight_panel.rs`](../../examples/apps/disco-demo/src/backlight_panel.rs) | shared `BacklightPanel` slider widget | `(backlight_panel.rs:NN)` |
| [`examples/apps/disco-demo/src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) | `activate_settings(Backlight)`, `cycle_backlight`, slider→command bridge | `(disco-demo/lib.rs:NN)` |
| Linux `panel-raspberrypi-touchscreen.c` | bridge `REG_PWM` name | `(pi-panel)` |

## §1 Purpose

Make the panel backlight adjustable from the UI: route the controller's
`DiscoCommand::SetBacklight(u8)` through a Rust→C hook to the DFR0550
bridge's PWM register, and — on pointer platforms — give it a continuous
slider rather than a discrete step. This is the first time the DFR0550 PWM
register was verified to actually *dim* the panel (previously only ever
written `255` at wake).

## §2 Problem statement

1. **Abstract level vs. hardware register.** `SetBacklight` carries an
   abstract `0..=100` level; the bridge wants `0..=255` in `REG_PWM`.
   Something on the C side, which owns all bridge I2C, must do the map.
2. **Slider vs. discrete step.** A continuous slider is only usable with a
   pointer. Keyboard/headless platforms (no pointer) would be stuck with
   an unoperable control, so the activation must branch on
   `DiscoCapabilities::pointer`.

## §3 Glossary

**Backlight hook** (`rlvgl_host_set_backlight`) and `DiscoCommand` are
defined in [BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md) and used without
modification. **`BacklightPanel`** is defined in
`(disco-demo/src/backlight_panel.rs)`; canonical in the shared crate, used
here without modification. This chapter adds no new vocabulary.

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| `SetBacklight` level → `REG_PWM` value | C host (`rlvgl_host_set_backlight`, `idf_compare.c:187`) |
| `SetBacklight` drain on P4 | Rust payload (`rlvgl_app/lib.rs:349-355`) |
| Slider widget | `disco-demo` `BacklightPanel` (shared; code is canonical) |
| Slider-vs-cycle decision | `disco-demo` `activate_settings(Backlight)` (`disco-demo/lib.rs:677-694`) |
| Slider value → `SetBacklight` | `disco-demo` controller bridge (`disco-demo/lib.rs:1386-1392`) |

## §5 Authority relationship matrix

| External authority | Concept | Relationship | Mutation rights | Divergence policy |
|---|---|---|---|---|
| `examples/apps/disco-demo/` | `BacklightPanel`, `cycle_backlight` | compose | none — shared with DISCO + BBB | upstream changes land at next rebuild |
| `panel-raspberrypi-touchscreen.c` | `REG_PWM` name | mirror | none | register name matches the kernel driver |

## §6 Frozen enums

None. `DiscoCommand::SetBacklight(u8)` is frozen in disco-demo; no variant
is added.

## §7 Frozen timing & topology

- **Level→PWM map:** clamp to 100, then `pwm = level * 255 / 100`;
  `bridge_write(s_bridge, DFR0550_REG_PWM, pwm)`; log
  `"Backlight NN% -> PWM NNN"` (`idf_compare.c:187-197`). The C host
  exposes `s_bridge` to the hook at `(idf_compare.c:312)`.
- **No synchronization needed:** the hook runs on the same FreeRTOS
  render task as every other bridge I2C access (`idf_compare.c:184-185`);
  it resolves at link as a non-static C symbol (Rust decl
  `rlvgl_app/lib.rs:45-48`, call `rlvgl_app/lib.rs:349-354`).
- **Settings slot:** Backlight is **slot 4** (the bug icon) in the left
  Settings wing; landscape bounds x < 70, y ≈ 292–362
  (`wing_slot_bounds`, `disco-demo/lib.rs:277-295`; `SettingsSlot::Backlight = 4`,
  `disco-demo/lib.rs:210`).

## §9 Frozen invariants

This chapter implements the backlight half of the IDF-hybrid surface; it
mints no new concepts-gate invariant. The relevant freeze is the
**backlight hook** glossary entry and the C-ABI link contract: the Rust
`extern "C" { fn rlvgl_host_set_backlight(level: u8); }` declaration and
the C `void rlvgl_host_set_backlight(uint8_t level)` definition MUST agree
(same discipline as INV-BEETLE-IDF-1, applied to the host-provided
direction).

## §10 Reconciliation vs adjacent repo primitives

- **Slider landed in the shared crate, not the P4 payload.** The one
  shared-crate change in this track's history — `BacklightPanel` — was
  added to `examples/apps/disco-demo/` so *all* pointer platforms benefit,
  rather than being forked into the P4 payload. This is the reconciliation
  point flagged in [BEETLE-IDF-00 §10](BEETLE-IDF-00-CONCEPTS.md).
- **Discrete vs. slider, decided in shared code.** `activate_settings(Backlight)`
  shows the slider panel if `capabilities.pointer`, else calls
  `cycle_backlight()` (25/50/75/100). The `b` hotkey *always* cycles
  (`disco-demo/lib.rs:677-694, 704-714, 889`). The P4 advertises
  `pointer: true`, so it gets the slider.
- **`BacklightPanel` conventions.** It wraps a `rlvgl_widgets::slider::Slider`
  over `0..=100` (tap-to-position, **not** drag) plus a `%` readout,
  follows the `DashboardPanel` visibility convention (hidden by default,
  zero bounds + draws nothing until `show()`), and exposes `take_pending()`
  for the controller to drain a value change into `SetBacklight`
  (`backlight_panel.rs:37-45, 99-102, 104-164`; controller bridge
  `disco-demo/lib.rs:1386-1392`).

## §11 Non-goals

- A P4-side slider fork (avoided on purpose; see §10).
- Gamma / non-linear brightness curves. The map is a flat
  `level * 255 / 100`.
- Persisting backlight across reboots.

## §12 Acceptance checklist

- [x] (a) `SetBacklight` is drained on P4 and forwarded to the host hook
      (`rlvgl_app/lib.rs:349-354`).
- [x] (b) The host hook clamps, maps to PWM, and writes `REG_PWM`
      (`idf_compare.c:187-197`); the hook resolves at link.
- [x] (c) The PWM register *dims* the panel — HIL log
      `Backlight 62% -> PWM 158` (first verified dim, 2026-06-15).
- [x] (d) Pointer platforms show the shared `BacklightPanel` slider;
      pointerless platforms keep `cycle_backlight`
      (`disco-demo/lib.rs:677-694`).
- [x] (e) disco-demo carries regression tests:
      `backlight_item_shows_slider_panel_on_pointer_platform`,
      `backlight_slider_tap_emits_set_backlight`,
      `backlight_item_cycles_on_pointerless_platform`
      (`disco-demo/lib.rs:1851-1927`).

> **UX note (informative).** During HIL the Backlight item (slot 4, the
> bug icon at x < 70, y ≈ 292–362) was hard to hit — "tapped all over and
> hit various icons"; disabled icons render *dimmed, not hidden*, adding
> targeting confusion. This was a targeting problem, not a code bug;
> confirmed working once the user "found it."

## §13 Files cited

- `examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c` (`rlvgl_host_set_backlight`)
- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs` (`SetBacklight` drain)
- `examples/apps/disco-demo/src/backlight_panel.rs` (shared slider)
- `examples/apps/disco-demo/src/lib.rs` (`activate_settings`, `cycle_backlight`, tests)

## §14 Unblocks

- Closes the M1–M5 conformance gate: a conforming IDF-hybrid deployment
  satisfies chapters 01–04 ([README §Conformance targets](README.md)).
- [BEETLE-IDF-05](BEETLE-IDF-05-STAR-CRAWL.md) (M6) — the remaining
  drained-and-dropped `StartEffect(StarCrawl)` command gains a renderer.

## §15 Change log

- **2026-06-19** (ratified retroactively) — documents work that shipped on
  the v0.2.4 branch (BEETLE M1/M3/M4/M5), merged to main in #216 /
  `5187ce0`.

---

**[← BEETLE-IDF-03](BEETLE-IDF-03-DISCO-DEMO.md) · [Index](README.md) · [BEETLE-IDF-05 →](BEETLE-IDF-05-STAR-CRAWL.md)**
