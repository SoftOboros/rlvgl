# ratatui-rlvgl

`ratatui-rlvgl` connects Ratatui's retained cell model to an rlvgl widget
tree. It provides a `no_std + alloc` Ratatui backend and an rlvgl widget that
paints the backend's shared cell surface through any rlvgl renderer.

```rust,ignore
use ratatui_core::terminal::Terminal;
use ratatui_rlvgl::{CellMetrics, RatatuiView, RlvglBackend};

// `bounds` is the content rectangle chosen by the enclosing application.
let (backend, surface) = RlvglBackend::new(80, 24, CellMetrics::packed_terminal())?;
let mut terminal = Terminal::new(backend)?;
let view = RatatuiView::new(bounds, surface);
```

## Owned source and dependency identity

The 0.1.2 maintenance source lives in the RLVGL repository at
`crates/ratatui-rlvgl`. Its upstream authority is official, unmodified Ratatui
0.30.2, commit `e665c36cb14752a61cd777fbd06dbef8474f2add`. The native
`ratatui-core` package is version 0.1.2; its package version is different from
the upstream Ratatui release number.

The following registry recipe is for the owned 0.1.2 package **after that
version is published**. A candidate manifest or source check does not establish
registry availability:

```toml
[dependencies]
ratatui-core = { version = "=0.1.2", default-features = false }
ratatui-rlvgl = { version = "0.1.2", default-features = false }
rlvgl-core = { version = "0.2.7", default-features = false }
```

In the RLVGL source checkout, the owned bridge is a regular root workspace
member and its native core dependency comes from the pinned official
`vendor/ratatui/ratatui-core` source. The former fork-owned
`vendor/ratatui/ratatui-rlvgl` carrier is historical. Keep one native core
identity throughout the application; do not mix the source core with a second
registry copy or use the old fork facade to recover its widget extensions.
Packaged consumers use normalized registry/extracted dependencies, without a
source/vendor fallback.

## Scoped compatibility and portable targets

Use `ratatui_core` for `Terminal`, `Frame`, `Buffer`, `Rect`, `Style`, text
primitives and the native `Widget` trait. The owned `ratatui_rlvgl::compat`
module supplies the current application's bounded compatibility:

| API | Scope |
| --- | --- |
| `CellEffect`, `Shadow`, `WithShadow` | Cell effects and shadow composition, including portable shared custom effects |
| `Borders`, `Panel` | Background, plain `NONE`/`ALL` borders and a left title; `inner` computes the content rectangle |
| `TextPanel` | Hard-line, left-aligned native text and styles, with clipping; no wrapping, scrolling or `.block(...)` builder |
| `BarValueLabel` | Display-column-centered label over caller-owned bar content; it does not draw the bar body or replace native `BarChart` |

These are not complete replacements for upstream `Block`, `Paragraph` or
widget-facade APIs. Display-width handling does not add CJK glyph coverage to
the curated fonts.

On targets without pointer atomics, enable the bridge's `portable-atomic`
feature. It forwards the matching feature to native core and uses portable
shared ownership for custom effects. An application exposing its own feature
can forward to both declared dependencies, as the SCTD and battery examples do:

```toml
[features]
portable-atomic = ["ratatui-core/portable-atomic", "ratatui-rlvgl/portable-atomic"]
```

The allocator and board's atomic/critical-section support remain application
prerequisites. The existing C3 single-core configuration is not a general
assumption for other hardware. This feature does not make the retained
`Rc<RefCell<_>>` surface a cross-thread framebuffer. Host tests or a Rust target
check do not qualify linked firmware or hardware operation.

The backend performs no device I/O. A Ratatui terminal writes cell updates to
the shared surface; `RatatuiView` draws that surface when it participates in
an rlvgl widget tree. The view can be the main content widget or a child pane
inside a native graphical screen.

`RatatuiView` defaults to the RATATUI-00 curated-Unicode, anti-aliased
`PackedFont` terminal family (`ratatui_rlvgl::fonts`): four variants
(regular/bold/oblique/bold-oblique) packed from `DejaVu Sans Mono`, covering
ASCII, the full box-drawing block (U+2500–U+257F), block elements
(U+2580–U+259F), the full arrow block (U+2190–U+21FF), and a small
status-symbol set (`●○■□✓✗▪▸`). `Modifier::BOLD` and `Modifier::ITALIC`
select a real font-variant outline instead of a synthetic redraw or an
upright no-op. `Modifier::SLOW_BLINK`/`Modifier::RAPID_BLINK` are driven by
an explicit, caller-triggered `RatatuiSurface::advance_blink_phase()` call —
never a wall clock — which suppresses the glyph (background only) on the
"off" phase; wire it to whatever tick source the host application already
owns. Glyphs outside the curated repertoire (Braille, CJK, and everything
else not enumerated above) still degrade through the existing ASCII
substitution (`+`, `-`, `|`) or `?` fallback. Ratatui named colors and
indexed colors use the same xterm 256-color palette; RGB values pass through
unchanged. Reversed, hidden, underlined, crossed-out, and dim modifiers have
static graphical mappings and compose with whichever font variant is
selected.

Call `RatatuiView::set_font_family(RatatuiTerminalFont::Bitmap6x10)` to opt
back into rlvgl's deterministic 1-bit `FONT_6X10` rasterizer and a fixed
12×20-pixel grid (`CellMetrics::font_6x10()`) — the lowest-flash-cost
profile, and a regression-proof baseline: in this mode every box-drawing
glyph still degrades to ASCII, bold is still a redrawn-one-pixel-right
approximation, and italic remains an upright static equivalent. Caller-driven
blink applies to both bitmap and packed families through
`RatatuiSurface::advance_blink_phase()`; the off phase paints background only.
Pair the default `Packed` family with `CellMetrics::packed_terminal()`
(14×21); pair `Bitmap6x10` with `CellMetrics::font_6x10()` (12×20).

The backend targets fullscreen and fixed viewports only —
`Viewport::Inline` and the `scrolling-regions` feature's scroll-region
methods are an intentional non-goal, not a gap: an embedded display has no
"scrollback above the viewport" surface for inline mode to be correct about.
See `RATATUI-00-CONCEPTS.md` §9 for the ratified rationale.

## Package contents and notices

The crate includes all four packed terminal font binaries used by `fonts.rs`.
They retain the existing regular, bold, oblique and bold-oblique byte sequences;
no font regeneration or repertoire expansion is involved.

The Rust bridge code retains its MIT license in `LICENSE`. The packed glyphs
are derived from DejaVu Sans Mono; their Bitstream/DejaVu/Arev notices are
included in `FONT-NOTICE.txt`.
