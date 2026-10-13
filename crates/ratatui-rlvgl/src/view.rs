//! rlvgl widget view and input translation.

use alloc::boxed::Box;
use core::cell::{Cell as CellSlot, RefCell};

use ratatui_core::style::Modifier;
use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::event::{Event, Key};
use rlvgl_core::packed_font::PackedFont;
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::widget::{Color, Rect, Widget};
use unicode_width::UnicodeWidthStr;

use crate::backend::{CellMetrics, RatatuiSurface};
use crate::color::{dim, resolve};
use crate::fonts;

type InputCallback = Box<dyn FnMut(RlvglInput)>;

#[derive(Clone, Copy)]
struct PaintContext {
    bounds: Rect,
    metrics: CellMetrics,
    defaults: crate::SurfaceDefaults,
    font_family: RatatuiTerminalFont,
    blink_phase: bool,
}

/// Terminal font family selected by [`RatatuiView`] for glyph rendering
/// (RATATUI-00 §6.2, §7).
#[derive(Clone, Copy)]
pub enum RatatuiTerminalFont {
    /// The deterministic 1-bit `FONT_6X10` baseline (pre-RATATUI-00
    /// behavior).
    ///
    /// In this mode `draw_cell` behaves byte-for-byte identically to the
    /// pre-RATATUI-00 implementation: box-drawing and other non-ASCII
    /// glyphs still degrade through [`bitmap_font_fallback`], bold is still
    /// the legacy pixel-offset redraw hack, and italic/blink glyph
    /// selection stays static (upright, non-animated). This is the
    /// explicit low-flash-cost opt-out RATATUI-00 §6.2 requires — a
    /// regression-proof escape hatch, not a second code path to also
    /// modernize.
    Bitmap6x10,
    /// The RATATUI-00a curated-Unicode, anti-aliased `PackedFont` terminal
    /// family (default — see [`crate::fonts`]).
    ///
    /// `draw_cell` selects among the four variants by
    /// `(Modifier::BOLD, Modifier::ITALIC)` (RATATUI-00 §7) and renders
    /// through [`Renderer::draw_glyph`] at a fixed per-cell advance
    /// (RATATUI-00 §6, "Fixed-advance rendering").
    Packed {
        /// Upright, normal-weight glyphs.
        regular: &'static PackedFont,
        /// Bold-weight glyphs.
        bold: &'static PackedFont,
        /// Oblique (italic-equivalent) glyphs.
        oblique: &'static PackedFont,
        /// Bold + oblique glyphs.
        bold_oblique: &'static PackedFont,
    },
}

impl RatatuiTerminalFont {
    /// The default curated-Unicode `PackedFont` family bundled at
    /// [`crate::fonts`].
    const fn default_packed() -> Self {
        Self::Packed {
            regular: &fonts::TERMINAL_REGULAR,
            bold: &fonts::TERMINAL_BOLD,
            oblique: &fonts::TERMINAL_OBLIQUE,
            bold_oblique: &fonts::TERMINAL_BOLD_OBLIQUE,
        }
    }
}

/// Backend-neutral key vocabulary emitted by [`RatatuiView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKey {
    /// Escape.
    Escape,
    /// Enter/return.
    Enter,
    /// Space.
    Space,
    /// Backspace.
    Backspace,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Function key number.
    Function(u8),
    /// Printable character.
    Character(char),
    /// Unclassified platform key code.
    Other(u32),
}

/// Pointer transition translated from an rlvgl event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    /// Raw pointer down.
    Down,
    /// Raw pointer up.
    Up,
    /// Raw pointer movement.
    Move,
    /// Debounced press began.
    Press,
    /// Debounced press released.
    Release,
}

/// Input delivered by an [`RatatuiView`] to its consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RlvglInput {
    /// Logical UI tick.
    Tick,
    /// Key press.
    KeyDown(InputKey),
    /// Key release.
    KeyUp(InputKey),
    /// Pointer activity inside the view.
    Pointer {
        /// Transition kind.
        kind: PointerKind,
        /// Local pixel coordinates relative to the view.
        pixel: (u16, u16),
        /// Derived Ratatui cell coordinates.
        cell: (u16, u16),
    },
}

/// rlvgl widget that paints a shared Ratatui cell surface.
pub struct RatatuiView {
    bounds: Rect,
    surface: RatatuiSurface,
    input: RefCell<Option<InputCallback>>,
    font_family: CellSlot<RatatuiTerminalFont>,
}

impl RatatuiView {
    /// Create a view at `bounds` for `surface`.
    ///
    /// Defaults to the RATATUI-00 curated-Unicode `PackedFont` terminal
    /// family; call [`set_font_family`](Self::set_font_family) with
    /// [`RatatuiTerminalFont::Bitmap6x10`] to opt back into the
    /// deterministic 1-bit baseline.
    pub fn new(bounds: Rect, surface: RatatuiSurface) -> Self {
        Self {
            bounds,
            surface,
            input: RefCell::new(None),
            font_family: CellSlot::new(RatatuiTerminalFont::default_packed()),
        }
    }

    /// Replace the optional input callback.
    pub fn set_on_input(&self, callback: Option<InputCallback>) {
        *self.input.borrow_mut() = callback;
    }

    /// Replace the terminal font family used to paint glyphs (RATATUI-00
    /// §6.2, §7).
    pub fn set_font_family(&self, font_family: RatatuiTerminalFont) {
        self.font_family.set(font_family);
    }

    /// Access the shared surface.
    pub const fn surface(&self) -> &RatatuiSurface {
        &self.surface
    }

    /// Convert the surface's dirty-cell union to screen pixel coordinates.
    pub fn dirty_pixel_rect(&self) -> Option<Rect> {
        let metrics = self.surface.metrics();
        let cells = self.surface.dirty_cells()?;
        Some(Rect {
            x: self
                .bounds
                .x
                .saturating_add(i32::from(cells.x).saturating_mul(i32::from(metrics.width()))),
            y: self
                .bounds
                .y
                .saturating_add(i32::from(cells.y).saturating_mul(i32::from(metrics.height()))),
            width: i32::from(cells.width).saturating_mul(i32::from(metrics.width())),
            height: i32::from(cells.height).saturating_mul(i32::from(metrics.height())),
        })
    }

    fn emit(&self, input: RlvglInput) -> bool {
        let mut callback = self.input.borrow_mut();
        if let Some(callback) = callback.as_mut() {
            callback(input);
            true
        } else {
            false
        }
    }

    fn pointer(&self, kind: PointerKind, x: i32, y: i32) -> bool {
        let Some(local_x) = x.checked_sub(self.bounds.x) else {
            return false;
        };
        let Some(local_y) = y.checked_sub(self.bounds.y) else {
            return false;
        };
        if local_x < 0
            || local_y < 0
            || local_x >= self.bounds.width
            || local_y >= self.bounds.height
        {
            return false;
        }
        let metrics = self.surface.metrics();
        let local_x = local_x.min(i32::from(u16::MAX)) as u16;
        let local_y = local_y.min(i32::from(u16::MAX)) as u16;
        self.emit(RlvglInput::Pointer {
            kind,
            pixel: (local_x, local_y),
            cell: (local_x / metrics.width(), local_y / metrics.height()),
        })
    }
}

impl Widget for RatatuiView {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let state = self.surface.borrow();
        if !state.published {
            return;
        }
        let metrics = state.metrics;
        let context = PaintContext {
            bounds: self.bounds,
            metrics,
            defaults: state.defaults,
            font_family: self.font_family.get(),
            blink_phase: state.blink_phase,
        };
        let mut clipped = ClipRenderer::new(renderer, self.bounds);
        let width = state.buffer.area.width;
        let height = state.buffer.area.height;
        let mut y = 0;
        while y < height {
            let mut x = 0;
            while x < width {
                let Some(cell) = state.buffer.cell((x, y)) else {
                    break;
                };
                let span = UnicodeWidthStr::width(cell.symbol())
                    .max(1)
                    .min(usize::from(width - x));
                draw_cell(&mut clipped, context, x, y, span as u16, cell);
                x = x.saturating_add(span as u16);
            }
            y += 1;
        }
        if state.cursor_visible && state.cursor.x < width && state.cursor.y < height {
            let x = self.bounds.x.saturating_add(
                i32::from(state.cursor.x).saturating_mul(i32::from(metrics.width())),
            );
            let y = self.bounds.y.saturating_add(
                i32::from(state.cursor.y).saturating_mul(i32::from(metrics.height())),
            );
            clipped.fill_rect(
                Rect {
                    x,
                    y: y.saturating_add(i32::from(metrics.height()))
                        .saturating_sub(2),
                    width: i32::from(metrics.width()),
                    height: 2,
                },
                state.defaults.foreground,
            );
        }
        drop(state);
        self.surface.mark_clean();
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Tick => self.emit(RlvglInput::Tick),
            Event::KeyDown { key } => self.emit(RlvglInput::KeyDown(map_key(key))),
            Event::KeyUp { key } => self.emit(RlvglInput::KeyUp(map_key(key))),
            Event::PointerDown { x, y } => self.pointer(PointerKind::Down, *x, *y),
            Event::PointerUp { x, y } => self.pointer(PointerKind::Up, *x, *y),
            Event::PointerMove { x, y } => self.pointer(PointerKind::Move, *x, *y),
            Event::PressDown { x, y } => self.pointer(PointerKind::Press, *x, *y),
            Event::PressRelease { x, y } => self.pointer(PointerKind::Release, *x, *y),
            _ => false,
        }
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

fn draw_cell(
    renderer: &mut dyn Renderer,
    context: PaintContext,
    x: u16,
    y: u16,
    span: u16,
    cell: &ratatui_core::buffer::Cell,
) {
    let PaintContext {
        bounds,
        metrics,
        defaults,
        font_family,
        blink_phase,
    } = context;
    let px = bounds
        .x
        .saturating_add(i32::from(x).saturating_mul(i32::from(metrics.width())));
    let py = bounds
        .y
        .saturating_add(i32::from(y).saturating_mul(i32::from(metrics.height())));
    let mut foreground = resolve(cell.fg, defaults.foreground);
    let mut background = resolve(cell.bg, defaults.background);
    if cell.modifier.contains(Modifier::REVERSED) {
        core::mem::swap(&mut foreground, &mut background);
    }
    if cell.modifier.contains(Modifier::DIM) {
        foreground = dim(foreground);
    }
    let cell_rect = Rect {
        x: px,
        y: py,
        width: i32::from(metrics.width()).saturating_mul(i32::from(span)),
        height: i32::from(metrics.height()),
    };
    renderer.fill_rect(cell_rect, background);
    let mut clipped = ClipRenderer::new(renderer, cell_rect);
    // RATATUI-00 §8: a blinking cell's glyph is suppressed (background
    // only) on the "off" phase. Orthogonal to font selection — applies in
    // both Bitmap6x10 and Packed modes.
    let blink_suppressed = cell
        .modifier
        .intersects(Modifier::SLOW_BLINK | Modifier::RAPID_BLINK)
        && !blink_phase;
    if !cell.modifier.contains(Modifier::HIDDEN) && !blink_suppressed {
        let origin = GlyphOrigin {
            px,
            py,
            metrics,
            span,
        };
        match font_family {
            RatatuiTerminalFont::Bitmap6x10 => {
                draw_cell_bitmap6x10(&mut clipped, origin, cell, foreground);
            }
            RatatuiTerminalFont::Packed {
                regular,
                bold,
                oblique,
                bold_oblique,
            } => {
                draw_cell_packed(
                    &mut clipped,
                    origin,
                    cell,
                    foreground,
                    [regular, bold, oblique, bold_oblique],
                );
            }
        }
    }
    if cell.modifier.contains(Modifier::UNDERLINED) {
        clipped.fill_rect(
            Rect {
                x: px,
                y: py
                    .saturating_add(i32::from(metrics.height()))
                    .saturating_sub(1),
                width: cell_rect.width,
                height: 1,
            },
            foreground,
        );
    }
    if cell.modifier.contains(Modifier::CROSSED_OUT) {
        clipped.fill_rect(
            Rect {
                x: px,
                y: py.saturating_add(i32::from(metrics.height() / 2)),
                width: cell_rect.width,
                height: 1,
            },
            foreground,
        );
    }
}

/// Shared per-cell placement inputs for the font-specific glyph-draw
/// helpers below. Bundled into one `Copy` struct so neither helper needs a
/// clippy-unfriendly long parameter list.
#[derive(Clone, Copy)]
struct GlyphOrigin {
    px: i32,
    py: i32,
    metrics: CellMetrics,
    span: u16,
}

/// Legacy `FONT_6X10` glyph draw: byte-for-byte the pre-RATATUI-00
/// behavior, including the pixel-offset bold redraw hack and the
/// box-drawing-to-ASCII fallback for every non-ASCII glyph.
fn draw_cell_bitmap6x10(
    clipped: &mut ClipRenderer<'_>,
    origin: GlyphOrigin,
    cell: &ratatui_core::buffer::Cell,
    foreground: Color,
) {
    let GlyphOrigin {
        px,
        py,
        metrics,
        span,
    } = origin;
    let glyph_y = py
        .saturating_add(i32::from(metrics.baseline()))
        .saturating_sub(FONT_6X10.scaled_height());
    for (index, ch) in cell
        .symbol()
        .chars()
        .take(usize::from(span))
        .map(bitmap_font_fallback)
        .enumerate()
    {
        FONT_6X10.draw_char(
            clipped,
            px.saturating_add((index as i32).saturating_mul(i32::from(metrics.width()))),
            glyph_y,
            ch,
            foreground,
        );
    }
    if cell.modifier.contains(Modifier::BOLD) {
        for (index, ch) in cell
            .symbol()
            .chars()
            .take(usize::from(span))
            .map(bitmap_font_fallback)
            .enumerate()
        {
            FONT_6X10.draw_char(
                clipped,
                px.saturating_add(1)
                    .saturating_add((index as i32).saturating_mul(i32::from(metrics.width()))),
                glyph_y,
                ch,
                foreground,
            );
        }
    }
}

/// RATATUI-00a/§6.2/§7: curated-Unicode, anti-aliased `PackedFont` glyph
/// draw. Selects among the four `[regular, bold, oblique, bold_oblique]`
/// variants by `(BOLD, ITALIC)`, falls back through
/// [`bitmap_font_fallback`] only when the curated repertoire genuinely
/// lacks a character, and steps by the fixed `metrics.width()` per glyph
/// (never the font's natural `advance_fp16`).
fn draw_cell_packed(
    clipped: &mut ClipRenderer<'_>,
    origin: GlyphOrigin,
    cell: &ratatui_core::buffer::Cell,
    foreground: Color,
    variants: [&'static PackedFont; 4],
) {
    let GlyphOrigin {
        px,
        py,
        metrics,
        span,
    } = origin;
    let [regular, bold, oblique, bold_oblique] = variants;
    let font: &PackedFont = match (
        cell.modifier.contains(Modifier::BOLD),
        cell.modifier.contains(Modifier::ITALIC),
    ) {
        (false, false) => regular,
        (true, false) => bold,
        (false, true) => oblique,
        (true, true) => bold_oblique,
    };
    let baseline_y = py.saturating_add(i32::from(metrics.baseline()));
    for (index, ch) in cell.symbol().chars().take(usize::from(span)).enumerate() {
        // RATATUI-00 §6.3 fallback policy: only degrade through
        // `bitmap_font_fallback` when the curated repertoire genuinely
        // lacks `ch` — never pre-degrade a glyph the PackedFont family
        // already draws.
        let resolved = if font.glyph(ch).is_some() {
            ch
        } else {
            bitmap_font_fallback(ch)
        };
        clipped.draw_glyph(
            font,
            resolved,
            (
                px.saturating_add((index as i32).saturating_mul(i32::from(metrics.width()))),
                baseline_y,
            ),
            foreground,
        );
    }
}

/// Preserve Ratatui's box layout when the selected embedded bitmap font only
/// contains ASCII. Richer rlvgl fonts can replace this policy in a future
/// configurable cell renderer without changing the backend contract.
const fn bitmap_font_fallback(character: char) -> char {
    match character {
        '─' | '━' | '═' | '╌' | '╍' | '┄' | '┅' | '┈' | '┉' => '-',
        '│' | '┃' | '║' | '╎' | '╏' | '┆' | '┇' | '┊' | '┋' => '|',
        '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '╔' | '╗' | '╚' | '╝' | '╠' | '╣'
        | '╦' | '╩' | '╬' => '+',
        character if character.is_ascii() => character,
        _ => '?',
    }
}

const fn map_key(key: &Key) -> InputKey {
    match key {
        Key::Escape => InputKey::Escape,
        Key::Enter => InputKey::Enter,
        Key::Space => InputKey::Space,
        Key::Backspace => InputKey::Backspace,
        Key::ArrowUp => InputKey::Up,
        Key::ArrowDown => InputKey::Down,
        Key::ArrowLeft => InputKey::Left,
        Key::ArrowRight => InputKey::Right,
        Key::Function(number) => InputKey::Function(*number),
        Key::Character(character) => InputKey::Character(*character),
        Key::Other(code) => InputKey::Other(*code),
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use core::cell::RefCell;

    use ratatui_core::backend::Backend;
    use ratatui_core::buffer::Cell;
    use rlvgl_core::renderer::Renderer;
    use rlvgl_core::widget::Color;

    use super::*;
    use crate::{CellRect, RlvglBackend, SurfaceDefaults};

    #[derive(Default)]
    struct RecordingRenderer {
        fills: alloc::vec::Vec<(Rect, Color)>,
        /// `(x, y, coverage_len)` for every [`Renderer::blend_row`] call —
        /// captured *before* per-pixel alpha filtering, so a glyph row's
        /// starting x position is observable even where every pixel in
        /// that row happens to have zero coverage. Used to assert
        /// fixed-advance glyph placement deterministically.
        glyph_rows: alloc::vec::Vec<(i32, i32, usize)>,
    }

    impl Renderer for RecordingRenderer {
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.fills.push((rect, color));
        }

        fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}

        fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
            self.glyph_rows.push((x, y, coverage.len()));
            for (index, &coverage) in coverage.iter().enumerate() {
                if coverage == 0 {
                    continue;
                }
                let alpha = (u16::from(color.3) * u16::from(coverage) / 255) as u8;
                self.fill_rect(
                    Rect {
                        x: x + index as i32,
                        y,
                        width: 1,
                        height: 1,
                    },
                    Color(color.0, color.1, color.2, alpha),
                );
            }
        }
    }

    #[test]
    fn view_draws_background_and_glyph_pixels() {
        let (mut backend, surface) = RlvglBackend::new(2, 1, CellMetrics::font_6x10()).unwrap();
        let cell = Cell::new("A");
        backend.draw([(0, 0, &cell)].into_iter()).unwrap();
        backend.flush().unwrap();
        let view = RatatuiView::new(
            Rect {
                x: 10,
                y: 20,
                width: 24,
                height: 20,
            },
            surface,
        );
        // Pin this test to the pre-RATATUI-00 FONT_6X10 baseline; the
        // curated-Unicode PackedFont default is covered by the
        // `packed_mode_*` tests below.
        view.set_font_family(RatatuiTerminalFont::Bitmap6x10);
        let mut renderer = RecordingRenderer::default();
        view.draw(&mut renderer);
        assert!(renderer.fills.iter().any(|(rect, _)| *rect
            == Rect {
                x: 10,
                y: 20,
                width: 12,
                height: 20
            }));
        assert!(renderer.fills.len() > 2, "glyph pixels should be drawn");
        let foreground = SurfaceDefaults::default().foreground;
        assert!(
            renderer
                .fills
                .iter()
                .any(|(rect, color)| *color == foreground && rect.y < 30),
            "glyph must occupy the upper half of its 20-pixel cell"
        );
    }

    #[test]
    fn dirty_cells_convert_to_view_pixels() {
        let (mut backend, surface) = RlvglBackend::new(4, 3, CellMetrics::font_6x10()).unwrap();
        surface.mark_clean();
        let cell = Cell::new("X");
        backend.draw([(2, 1, &cell)].into_iter()).unwrap();
        backend.flush().unwrap();
        assert_eq!(
            surface.dirty_cells(),
            Some(CellRect {
                x: 2,
                y: 1,
                width: 1,
                height: 1
            })
        );
        let view = RatatuiView::new(
            Rect {
                x: 5,
                y: 7,
                width: 48,
                height: 60,
            },
            surface,
        );
        assert_eq!(
            view.dirty_pixel_rect(),
            Some(Rect {
                x: 29,
                y: 27,
                width: 12,
                height: 20
            })
        );
    }

    #[test]
    fn translates_pointer_to_local_pixels_and_cells() {
        let (_, surface) = RlvglBackend::new(4, 3, CellMetrics::font_6x10()).unwrap();
        let mut view = RatatuiView::new(
            Rect {
                x: 10,
                y: 20,
                width: 48,
                height: 60,
            },
            surface,
        );
        let seen = Rc::new(RefCell::new(None));
        let seen_callback = seen.clone();
        view.set_on_input(Some(Box::new(move |input| {
            *seen_callback.borrow_mut() = Some(input);
        })));
        assert!(view.handle_event(&Event::PressRelease { x: 35, y: 51 }));
        assert_eq!(
            *seen.borrow(),
            Some(RlvglInput::Pointer {
                kind: PointerKind::Release,
                pixel: (25, 31),
                cell: (2, 1)
            })
        );
        assert!(!view.handle_event(&Event::PressRelease { x: 70, y: 31 }));
    }

    #[test]
    fn maps_unicode_box_drawing_to_ascii_bitmap_glyphs() {
        assert_eq!(bitmap_font_fallback('─'), '-');
        assert_eq!(bitmap_font_fallback('│'), '|');
        assert_eq!(bitmap_font_fallback('┼'), '+');
        assert_eq!(bitmap_font_fallback('λ'), '?');
    }

    fn packed_paint_context(bounds: Rect) -> PaintContext {
        PaintContext {
            bounds,
            metrics: CellMetrics::packed_terminal(),
            defaults: SurfaceDefaults::default(),
            font_family: RatatuiTerminalFont::default_packed(),
            blink_phase: true,
        }
    }

    // RATATUI-00a/§12 acceptance (a): a full-width box-drawing border
    // renders through the curated font unbroken, not degraded to `-`.
    #[test]
    fn packed_mode_renders_curated_box_drawing_unbroken() {
        let box_glyph = fonts::TERMINAL_REGULAR
            .glyph('─')
            .expect("RATATUI-00 §6.3 curated repertoire includes U+2500");
        let dash_glyph = fonts::TERMINAL_REGULAR
            .glyph('-')
            .expect("ASCII '-' is in the curated repertoire");
        assert_ne!(
            box_glyph.width, dash_glyph.width,
            "fixture requires the box-drawing glyph and its ASCII fallback to differ in width"
        );

        let context = packed_paint_context(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 21,
        });
        let cell = Cell::new("─");
        let mut renderer = RecordingRenderer::default();
        draw_cell(&mut renderer, context, 0, 0, 1, &cell);

        assert!(
            !renderer.glyph_rows.is_empty(),
            "the box-drawing glyph must draw at least one coverage row"
        );
        assert!(
            renderer
                .glyph_rows
                .iter()
                .all(|&(_, _, len)| len == usize::from(box_glyph.width)),
            "glyph rows must span the curated box-drawing glyph's own width \
             ({}), not the ASCII fallback's ({})",
            box_glyph.width,
            dash_glyph.width
        );
    }

    // RATATUI-00b/§12 acceptance (b): a BOLD box-drawing cell renders with
    // unbroken glyphs, proving §6 (curated repertoire) and §7 (bold variant
    // selection) compose.
    #[test]
    fn packed_mode_bold_box_drawing_composes_unbroken() {
        let bold_box_glyph = fonts::TERMINAL_BOLD
            .glyph('─')
            .expect("the bold family packs the identical curated repertoire (RATATUI-00 §6.1)");

        let context = packed_paint_context(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 21,
        });
        let mut cell = Cell::new("─");
        cell.modifier.insert(Modifier::BOLD);
        let mut renderer = RecordingRenderer::default();
        draw_cell(&mut renderer, context, 0, 0, 1, &cell);

        assert!(!renderer.glyph_rows.is_empty());
        assert!(
            renderer
                .glyph_rows
                .iter()
                .all(|&(_, _, len)| len == usize::from(bold_box_glyph.width)),
            "BOLD must select the bold PackedFont variant and still render \
             the full-width glyph, not the legacy pixel-offset double-draw hack"
        );
    }

    // RATATUI-00a/§12 acceptance (item under "no silent regression"):
    // glyphs outside the curated repertoire (e.g. Braille) still degrade
    // through the existing `bitmap_font_fallback` policy.
    #[test]
    fn packed_mode_degrades_outside_curated_repertoire() {
        assert!(
            fonts::TERMINAL_REGULAR.glyph('\u{2800}').is_none(),
            "Braille (U+2800) is explicitly excluded from RATATUI-00 §6.3"
        );
        let fallback_ch = bitmap_font_fallback('\u{2800}');
        assert_eq!(fallback_ch, '?');
        let fallback_glyph = fonts::TERMINAL_REGULAR
            .glyph(fallback_ch)
            .expect("the ASCII fallback substitute must itself be curated");

        let context = packed_paint_context(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 21,
        });
        let cell = Cell::new("\u{2800}");
        let mut renderer = RecordingRenderer::default();
        draw_cell(&mut renderer, context, 0, 0, 1, &cell);

        assert!(
            !renderer.glyph_rows.is_empty(),
            "the fallback substitute must still render, not silently disappear"
        );
        assert!(
            renderer
                .glyph_rows
                .iter()
                .all(|&(_, _, len)| len == usize::from(fallback_glyph.width)),
            "an out-of-repertoire glyph must render its ASCII substitute, not attempt Braille directly"
        );
    }

    // RATATUI-00 §12 "all phases" / regression guard: RatatuiTerminalFont::
    // Bitmap6x10 reproduces the exact pre-RATATUI-00 behavior — box-drawing
    // still degrades to ASCII and BOLD still uses the pixel-offset hack.
    #[test]
    fn bitmap6x10_mode_matches_legacy_fallback_and_bold_hack() {
        let context = PaintContext {
            bounds: Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 20,
            },
            metrics: CellMetrics::font_6x10(),
            defaults: SurfaceDefaults::default(),
            font_family: RatatuiTerminalFont::Bitmap6x10,
            blink_phase: true,
        };

        let mut box_drawn = RecordingRenderer::default();
        draw_cell(&mut box_drawn, context, 0, 0, 1, &Cell::new("─"));
        let mut dash_drawn = RecordingRenderer::default();
        draw_cell(&mut dash_drawn, context, 0, 0, 1, &Cell::new("-"));
        assert_eq!(
            box_drawn.fills, dash_drawn.fills,
            "Bitmap6x10 mode must still degrade box-drawing to its ASCII substitute"
        );
        assert!(
            box_drawn.glyph_rows.is_empty(),
            "FONT_6X10 draws pixels via fill_rect directly, never blend_row"
        );

        let mut bold_a = RecordingRenderer::default();
        let mut bold_cell = Cell::new("A");
        bold_cell.modifier.insert(Modifier::BOLD);
        draw_cell(&mut bold_a, context, 0, 0, 1, &bold_cell);
        let mut plain_a = RecordingRenderer::default();
        draw_cell(&mut plain_a, context, 0, 0, 1, &Cell::new("A"));
        assert!(
            bold_a.fills.len() > plain_a.fills.len(),
            "Bitmap6x10 BOLD must still redraw glyph pixels a second time (legacy hack)"
        );
    }

    // RATATUI-00c/§12 acceptance (e): advancing the blink phase flips
    // glyph visibility for a SLOW_BLINK cell, identically in both font
    // modes.
    #[test]
    fn blink_phase_suppresses_glyph_identically_in_both_font_modes() {
        let mut cell = Cell::new("A");
        cell.modifier.insert(Modifier::SLOW_BLINK);

        let modes = [
            (
                RatatuiTerminalFont::Bitmap6x10,
                CellMetrics::font_6x10(),
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 20,
                },
            ),
            (
                RatatuiTerminalFont::default_packed(),
                CellMetrics::packed_terminal(),
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 21,
                },
            ),
        ];

        for (font_family, metrics, bounds) in modes {
            let visible_context = PaintContext {
                bounds,
                metrics,
                defaults: SurfaceDefaults::default(),
                font_family,
                blink_phase: true,
            };
            let mut visible = RecordingRenderer::default();
            draw_cell(&mut visible, visible_context, 0, 0, 1, &cell);

            let hidden_context = PaintContext {
                blink_phase: false,
                ..visible_context
            };
            let mut hidden = RecordingRenderer::default();
            draw_cell(&mut hidden, hidden_context, 0, 0, 1, &cell);

            assert_eq!(
                hidden.fills.len(),
                1,
                "the 'off' phase must paint only the resolved background"
            );
            assert!(
                visible.fills.len() > hidden.fills.len(),
                "the 'on' phase must draw glyph pixels in addition to the background"
            );
        }
    }

    // RATATUI-00 §6 glossary "Fixed-advance rendering": consecutive glyphs
    // land exactly `metrics.width()` apart, ignoring the font's natural
    // `advance_fp16`.
    #[test]
    fn packed_mode_uses_fixed_cell_advance_not_natural_glyph_advance() {
        let metrics = CellMetrics::packed_terminal();
        let glyph = fonts::TERMINAL_REGULAR
            .glyph('─')
            .expect("curated repertoire includes U+2500");
        let natural_step = (i32::from(glyph.advance_fp16) + 8) >> 4;
        assert_ne!(
            natural_step,
            i32::from(metrics.width()),
            "fixture must exercise a natural advance that diverges from the fixed cell width"
        );

        let context = packed_paint_context(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 21,
        });
        let cell = Cell::new("──");
        let mut renderer = RecordingRenderer::default();
        draw_cell(&mut renderer, context, 0, 0, 2, &cell);

        let mut xs: alloc::vec::Vec<i32> = renderer.glyph_rows.iter().map(|&(x, _, _)| x).collect();
        xs.sort_unstable();
        xs.dedup();
        assert_eq!(
            xs,
            alloc::vec![0, i32::from(metrics.width())],
            "consecutive glyphs must land exactly metrics.width() apart, not at the font's natural advance"
        );
    }
}
