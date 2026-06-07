# DPR-01-A — disco-sim playit_automation triage

**Status:** 2026-05-19. Informative triage report scoped to the
`rlvgl-example-disco-sim` test crate; not a normative DPR-01 amendment.

## Findings

After `e67246c chore(workspace): unblock cargo test --workspace gate`
made the workspace-wide test gate runnable, four tests in
`examples/disco-sim/tests/playit_automation.rs` fail consistently. They
also fail under the per-crate invocation `cargo test -p
rlvgl-example-disco-sim --test playit_automation`, so the failure is
not workspace-mode-specific.

| # | Test | Failure mode | Class |
|---|---|---|---|
| 1 | `automation_headless_emits_ready_status_and_dump_frames` | `D0,0,4,2` returns all `00000000` ("frame dump was unexpectedly blank") | STALE_SETUP |
| 2 | `dashboard_rounded_corner_arcs_outward` | top row of dump at `D84,84,20,20` is all `(0,0,0)` ("no border pixels visible in top row of corner") | STALE_SETUP |
| 3 | `framebuffer_has_content_at_startup` | dump at `D0,0,40,20` is all `00000000` ("framebuffer was blank at startup") | STALE_SETUP |
| 4 | `transparent_label_backgrounds_do_not_blacken_window` | dump row at `D110,30,20,1` is all `(0,0,0)` ("title band at y=30 contains zero pixels") | STALE_SETUP |

All four are stale-setup failures caused by the *behaviour* of the
disco-demo / disco-sim rendering pipeline drifting after the tests were
written, not by any rendering-pipeline regression.

## Root cause analysis

Two independent assumptions baked into the original tests are no longer
true:

### (a) The dashboard panel starts hidden

Commit `504d56b refactor: dashboard starts hidden, rename Flight Deck
to About` (2026-04-13) changed
`examples/apps/disco-demo/src/dashboard_panel.rs:25-46`. Specifically:

```rust
pub struct DashboardPanel {
    bounds: Rect,
    /* ... */
    visible: bool,
}

impl DashboardPanel {
    pub fn new(bounds: Rect, title: ..., caption: ...) -> Self {
        Self { /* ... */ visible: false }
    }
}

impl Widget for DashboardPanel {
    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        fill_rounded_rect(renderer, self.bounds, PANEL_BG, PANEL_RADIUS);
        // ...
    }
}
```

The tests in commit `0e4ab8d Add playit-driven screen regression tests
for the recent fixes` (2026-04-10) were written **three days before**
the visibility refactor, when the panel was unconditionally drawn at
startup. The `dashboard_rounded_corner_arcs_outward` test reads pixels
at `(84, 84, 20, 20)` expecting the panel's top-left arc; with the
panel hidden, this region is the unwritten framebuffer (`vec![0; ...]`)
and the test fails.

The same hidden-panel state propagates into
`automation_headless_emits_ready_status_and_dump_frames` (dump at
`(0, 0)` — outside any rendered widget) and
`framebuffer_has_content_at_startup` (dump at `(0, 0, 40, 20)` — also
outside any rendered widget at startup; only the right-edge IconStrip
at `x ≈ 730..790, y ≈ 17..207` draws).

Path to make the panel visible: tap or key-activate the Info wing
(`KD:i`) then activate its first slot Diagnostics (`KD:Enter`). The
flow goes through `ControllerState::activate_info(InfoSlot::Diagnostics)`
→ `render_info_page(Diagnostics)` → `show_info(...)` →
`dashboard.borrow_mut().show()` at
`examples/apps/disco-demo/src/lib.rs:482`.

### (b) The disco-sim runtime has no window-background fill

`transparent_label_backgrounds_do_not_blacken_window` expects pixels at
`y=30` (inside the title-label band) to be the dark-navy "window
background" colour `Color(13, 19, 30) = 0xFF0D131E`. This colour is
never written into the framebuffer:

- The simulator runtime in `examples/disco-sim/src/main.rs:364-396`
  (`DiscoRuntime::render_frame`) creates a `BlitterRenderer` over the
  zero-initialised `FrameMirror::buf` and calls
  `self.root.borrow().draw(&mut renderer)`. No `fill_rect` is issued
  before the widget tree draws.
- The disco-demo root container at
  `examples/apps/disco-demo/src/lib.rs:864-877` is created with
  `bg_color(Color(0, 0, 0, 0))` (transparent — comment: "Transparent
  root — desktop/splash background shows through"). The hardware path
  has an underlying desktop layer, but the simulator does not.
- `themed_label()` at `examples/apps/disco-demo/src/lib.rs:1356-1364`
  sets `bg_color(Color(0, 0, 0, 0)).alpha(0)`, which
  `core::draw::draw_widget_bg` (correctly) treats as a no-op fill per
  the alpha-zero guard.

So in the current disco-sim rendering model the title/subtitle/footer
bands sit over zero pixels regardless of the alpha-zero fix. The
original regression (CpuBlitter overwriting destination pixels with
`0x00000000` when called with a transparent colour) is now covered
exclusively by `core::draw::tests::draw_widget_bg_alpha_zero_skipped`
in `core/src/draw.rs`. The simulator-level reproduction needs a
non-zero baseline behind the labels, which the current runtime never
produces.

## Recommended fix

Applied in this commit — test-code-only changes to
`examples/disco-sim/tests/playit_automation.rs`:

1. **`automation_headless_emits_ready_status_and_dump_frames`** — change
   the final dump from `D0,0,4,2,1` to `D760,90,4,2,1` (inside the
   IconStrip's first slot, which always renders).
2. **`framebuffer_has_content_at_startup`** — change the dump from
   `D0,0,40,20,1` to `D740,80,40,40,1` (IconStrip area).
3. **`dashboard_rounded_corner_arcs_outward`** — send `KD:i` + `KD:Enter`
   before the corner dump to navigate Info wing → Diagnostics, which
   calls `dashboard.show()`.
4. **`transparent_label_backgrounds_do_not_blacken_window`** — re-target
   the test. The original regression class is covered by host unit
   tests in `core::draw::tests`; the simulator-level assertion is
   strengthened to "the dashboard panel's opaque PANEL_BG fill renders
   as a non-zero colour after activation." A CpuBlitter regression that
   wrote zeros over opaque fills would also fail this assertion, so the
   intent of the original test is preserved.

Each modified test carries an inline comment citing this triage doc and
the relevant commit (`504d56b`).

The alternative — making `DashboardPanel` visible by default in the
simulator, or adding an opaque window-bg fill to `DiscoRuntime::render_frame`
— would touch `examples/apps/disco-demo/` or rendering code that the
triage task explicitly carved out of scope. The test-code patch is
both narrower and aligned with the post-`504d56b` user-facing
behaviour.

## Classification

| Test | Classification |
|---|---|
| `automation_headless_emits_ready_status_and_dump_frames` | STALE_SETUP |
| `dashboard_rounded_corner_arcs_outward` | STALE_SETUP |
| `framebuffer_has_content_at_startup` | STALE_SETUP |
| `transparent_label_backgrounds_do_not_blacken_window` | STALE_SETUP |

None of the four failures reflects a regression in the rendering
pipeline. All four assume a runtime state — visible dashboard panel
and/or non-zero window background — that the disco-sim binary has not
produced since commit `504d56b` (2026-04-13). The workspace-wide test
gate simply exposed the staleness once it could reach these tests for
the first time.

## Files cited

- `examples/disco-sim/tests/playit_automation.rs` — test patches
  (this commit).
- `examples/disco-sim/src/main.rs:364-396` —
  `DiscoRuntime::render_frame`; confirms no window-bg fill.
- `examples/apps/disco-demo/src/lib.rs:864-877` — root container
  with transparent bg.
- `examples/apps/disco-demo/src/lib.rs:1356-1364` — `themed_label`
  with alpha-zero bg.
- `examples/apps/disco-demo/src/dashboard_panel.rs:25-46, 119-122` —
  `visible: false` default and `draw()` early-return.
- `core/src/draw.rs:480-497` — `draw_widget_bg` alpha-zero guard
  (the invariant the original test was guarding).

## Follow-up: Node.js harness

The same stale-setup pattern affected three tests in the JS harness
under `playit/node/test/`, run via `node --test` in Phase 4.5 of
pre-publish (CLAUDE.md). Confirmed by running
`RLVGL_DISCO_SIM_BIN=<abs path> node --test` against the v0.2.0
disco-sim binary at this commit. The remaining seven Node-side tests
already sampled live IconStrip / wing regions and passed unchanged.

| # | File / test | Failure mode | Pattern |
|---|---|---|---|
| 1 | `disco-sim.test.js:10` `headless disco sim reports advancing status and exposes frame dumps` | `D0,0,6,4` returns zero pixels | Pattern 1 |
| 2 | `disco-navigation.test.js:76` `hotkey roundtrip: s, f, i, b all change controller state` | `D100,100,10,5` identical before/after `f` and before/after `b` | Pattern 1 |
| 3 | `disco-navigation.test.js:114` `framebuffer differs across main panels` | `D100,100,20,10` identical across settings/files/info states | Pattern 1 |

Patches are test-code-only:

- **Test 1** — retarget dump to IconStrip slot 1 interior at
  `(760, 90, 6, 4)`. Slot 1's icon (`ICON_FILE`) is always rendered at
  startup, so the dump is non-zero on the first present.
- **Test 2** — retarget both `f`-transition and `b`-transition dumps
  to `(740, 80, 40, 20)`, which spans the top border row of IconStrip
  slot 1 (y=87..89). `f` moves focus `Main(0) → Main(1)` (slot 1
  border appears); `b` moves focus `Main(1) → Wing(Settings, 4)`,
  which clears `strip_slot` and removes slot 1's border. Both
  transitions touch the sampled rows.
- **Test 3** — retarget the three per-panel dumps to the same
  `(740, 80, 40, 20)` region. Settings (slot 0 focused) → Files (slot
  1 focused) toggles slot 1's border on; Files → Info (slot 2 focused
  + info wing open) toggles it off. The two compared pairs both
  produce a pixel delta in the sample.

Playit's `D<x>,<y>,<w>,<h>` command caps width and height at 40 each
(`playit/src/protocol.rs:240-241`); the 40×20 stripe is the maximum
height that still cleanly straddles slot 1's top border without
spilling into slot 0. Pattern 2 (activation prologue `KD:i` + `KD:Enter`)
was not needed — focus-only transitions inside the IconStrip suffice
for all three Node-side failures.

No new failure modes (Pattern 4) observed; all three tests now pass
stably across three consecutive runs.

## Change log

- 2026-05-19 — initial triage; four STALE_SETUP failures in the Rust
  `playit_automation` suite fixed by test-code patches.
- 2026-05-19 — Node.js harness follow-up: three additional STALE_SETUP
  failures (`disco-sim.test.js`, `disco-navigation.test.js`) fixed by
  applying Pattern 1 (retarget to IconStrip slot 1 border row).
