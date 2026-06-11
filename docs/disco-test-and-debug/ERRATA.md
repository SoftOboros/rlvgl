<!--
ERRATA.md - disco-test-and-debug family errata log (simulator + test
automation surfaces). Triaged-and-accepted issues only; GH Issues is
the intake queue. Entries permanent across resolution as institutional
memory, per the BEETLE errata-log precedent.
-->

# Disco Test & Debug Errata Log

Triaged-and-accepted issues for the disco-sim / playit automation
surfaces (`examples/disco-sim/`, `rlvgl-playit`, headless render
paths in `rlvgl-platform`).

## Status legend

- 🟢 — Resolved. Fix has landed and verification evidence is recorded.
- 🟡 — Diagnosed. Root cause known; fix prescription written but not
  yet landed.
- 🔴 — Open. Symptom observed; root cause unknown.

## Entries

### DSIM-ERRATA-001 — 🟢 No text in the disco-sim CPU mirror (all modes)

*(filed 2026-06-10; resolved 2026-06-10)*

**Symptom (as reported):** `--headless=<path>` ASCII frames contain the
right-edge icon strip only — headline/subtitle/footer text absent.
`D` pixel dumps over text regions return background color. Affects
`--automation-headless`, `--headless`, `--png-path`, **and the windowed
mode** — all four paths render through the same `FrameMirror` buffer
(`DiscoRuntime::render_frame`, `examples/disco-sim/src/main.rs`), so
the windowed sim is equally text-free; this had not been noticed.

**Root cause (confirmed):** `examples/disco-sim/Cargo.toml` builds
`rlvgl-platform` with `features = ["simulator"]` — no `fontdue`.
Without the `fontdue` feature, `BlitterRenderer`'s `Renderer::draw_text`
impl is an explicit no-op stub (`platform/src/blit.rs`,
`#[cfg(not(feature = "fontdue"))] { let _ = (position, text, color); }`).
Every label/headline drawn by `DiscoController` silently vanishes.
Icons survive because the RLE icon path goes through `draw_pixels` /
blits, not `draw_text`.

**Prior art:** `consumers/user-sim` hit the identical trap during
CRATES-CI-02 bring-up and fixed it by adding `fontdue` to its
`rlvgl-platform` feature list (see `consumers/user-sim/Cargo.toml`
comment). The `simulator` feature does NOT imply `fontdue`; this is an
easy foot-gun for every simulator consumer.

**Fix prescription:** add `"fontdue"` to the `rlvgl-platform` feature
list in `examples/disco-sim/Cargo.toml` (both dependency stanzas).
Consider whether `simulator` SHOULD imply `fontdue` for all consumers —
that is a `platform/Cargo.toml` feature-graph decision with a size
cost on text-free consumers; record under CRATES-CI/§15 if taken.

**Resolution (2026-06-10):** `"fontdue"` added to both `rlvgl-platform`
feature stanzas in `examples/disco-sim/Cargo.toml`.

**Verification evidence:** `--headless` ASCII now shows letterform
glyph runs starting row 32 col ≈87 (headline) plus 25 rows of glyphs
outside the icon-strip columns (previously zero); background renders
as the `.` floor; playit node suite 9/9 pass against the fixed binary.

**Follow-up resolved (2026-06-10, owner decision):** `simulator` now
implies `fontdue` in `platform/Cargo.toml` as of **rlvgl-platform
0.2.3** — every known simulator consumer needs text, and the stub
shipped two invisible-text binaries (disco-sim, user-sim) before
anyone noticed. Consumers pinning registry versions ≤ 0.2.2 still
need the explicit `fontdue` feature (as `consumers/user-sim` carries).

### DSIM-ERRATA-002 — 🟢 "Missing background fill" is the dark theme, not a defect

*(filed 2026-06-10; resolved at triage)*

**Symptom (as reported):** ASCII cells read luminance 0 outside the
icon strip — interpreted as the background fill never reaching the
mirror, with the hypothesis that bg/text render only via wgpu.

**Finding:** the background IS in the mirror. `render_frame` clears to
`WINDOW_BG_ARGB8888 = 0xFF0D_131E` (`examples/disco-sim/src/main.rs:35`)
— a dark navy at ≈7% luminance, which the ASCII ramp maps to blank
cells. Widget panel fills also land: `Renderer::fill_rect` writes
immediately through `CpuBlitter::fill` (`platform/src/blit.rs:594`),
verified by `D` dumps returning `ff0d131e` (not `00000000`) across the
frame on current main. The wgpu path performs no widget rendering of
its own — it presents the same CPU buffer. The frame *looks* empty
because everything visible on the dark desktop other than icons is
text, and text is DSIM-ERRATA-001.

**Optional follow-up:** the ASCII dumper could print a floor marker
(e.g. `.`) for non-zero-but-dark pixels so "rendered dark" and
"never rendered" are distinguishable at a glance.

### DSIM-ERRATA-003 — 🟢 First-poll `D` dumps can capture an all-zero frame

*(filed 2026-06-10; resolved 2026-06-10)*

**Symptom (as reported):** `D` dumps return rows of `00000000` under
`--automation-headless`.

**Root cause (confirmed by inspection; timing-dependent to reproduce):**
`FrameMirror::new` zero-fills the buffer, and `DiscoRuntime::step()`
polls playit BEFORE rendering (`poll_playit()` → … → `render_frame()`).
A `D` command that arrives in the very first poll is captured against
the never-rendered zero buffer. From the second step onward dumps read
real pixels (verified on current main: status advances 1→17 and dumps
return `ff0d131e` background), so the zeros report reproduces only when
the client wins the race to the first poll.

**Fix prescription:** render once before serving — either call
`render_frame()` at the end of `DiscoRuntime::new`, or reorder the
first iteration of `run_automation_headless` (and the windowed frame
callback closure) to render before the initial poll. The
`present_count` exposed via `?` should never be 0 while the transport
is accepting commands.

**Resolution (2026-06-10):** `DiscoRuntime::new` renders one frame
before returning (`render_frame()` at construction), so the mirror is
populated before the transport serves its first poll.

**Verification evidence:** `D 0,0,8,1` issued immediately after
`PLAYIT_READY` (no status wait) returns `ff0d131e` background rows,
not `00000000`; first `?` reports `presentCount: 1`; playit node
suite 9/9 pass.
