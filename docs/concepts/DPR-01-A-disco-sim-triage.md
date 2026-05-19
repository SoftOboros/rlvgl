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

## Follow-up — playit node tests

The same stale-setup pattern affects the JS harness tests under
`playit/node/test/` (e.g. `disco-sim.test.js:25` dumps `(0,0,6,4)`,
which the post-`504d56b` runtime renders as zero pixels). These run
via `node --test` in Phase 4.5 of pre-publish (CLAUDE.md), not via
`cargo test --workspace`, so they were not part of this triage's
validation gate. The same surgical-dump-relocation fix applies — track
as a separate follow-up.

## Change log

- 2026-05-19 — initial triage; four STALE_SETUP failures fixed by
  test-code patches. Node-side equivalents flagged as follow-up.
