//! Retained Ratatui backend state.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Ref, RefCell};
use core::fmt;

use ratatui_core::backend::{Backend, ClearType, WindowSize};
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::{Position, Rect as TuiRect, Size};
use rlvgl_core::widget::Color;

/// Pixel geometry of one terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellMetrics {
    /// Cell width in pixels.
    width: u16,
    /// Cell height in pixels.
    height: u16,
    /// Text baseline offset from the cell's top edge.
    baseline: u16,
}

impl CellMetrics {
    /// Metrics matching rlvgl's built-in `FONT_6X10` bitmap font at its
    /// configured 2× display scale.
    pub const fn font_6x10() -> Self {
        Self {
            width: 12,
            height: 20,
            baseline: 20,
        }
    }

    /// Metrics matching the RATATUI-00 §6 curated-Unicode, anti-aliased
    /// `PackedFont` terminal family (see [`crate::fonts`]), packed from
    /// `DejaVu` Sans Mono at 17pt.
    ///
    /// Not 12×20 like [`Self::font_6x10`]: `DejaVu` Sans Mono's natural
    /// glyph-box proportions do not round cleanly onto that grid at any
    /// integer point size (RATATUI-00-CONCEPTS.md §6.1 explicitly allows a
    /// new constructor for this case). At 17pt, the curated repertoire's
    /// widest glyph — the oblique/bold-oblique variants' sheared `X` and
    /// several box-drawing/arrow glyphs — needs up to 14px of width, and
    /// the full vertical extent from the highest ascender (`│`) to the
    /// lowest descender ink (`_`) spans 21px around the baseline.
    pub const fn packed_terminal() -> Self {
        Self {
            width: 14,
            height: 21,
            baseline: 16,
        }
    }

    /// Cell width in pixels.
    pub const fn width(self) -> u16 {
        self.width
    }

    /// Cell height in pixels.
    pub const fn height(self) -> u16 {
        self.height
    }

    /// Text baseline offset from the cell's top edge.
    pub const fn baseline(self) -> u16 {
        self.baseline
    }
}

/// Default colors used to resolve Ratatui's `Color::Reset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceDefaults {
    /// Default foreground color.
    pub foreground: Color,
    /// Default background color.
    pub background: Color,
}

impl Default for SurfaceDefaults {
    fn default() -> Self {
        Self {
            foreground: Color(224, 231, 239, 255),
            background: Color(16, 22, 31, 255),
        }
    }
}

/// Cell-space rectangle used for dirty-region reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRect {
    /// Left column.
    pub x: u16,
    /// Top row.
    pub y: u16,
    /// Width in cells.
    pub width: u16,
    /// Height in cells.
    pub height: u16,
}

impl CellRect {
    const fn point(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            width: 1,
            height: 1,
        }
    }

    fn union(self, other: Self) -> Self {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = self
            .x
            .saturating_add(self.width)
            .max(other.x.saturating_add(other.width));
        let y2 = self
            .y
            .saturating_add(self.height)
            .max(other.y.saturating_add(other.height));
        Self {
            x: x1,
            y: y1,
            width: x2.saturating_sub(x1),
            height: y2.saturating_sub(y1),
        }
    }
}

pub(crate) struct SurfaceState {
    pub(crate) buffer: Buffer,
    pub(crate) metrics: CellMetrics,
    pub(crate) defaults: SurfaceDefaults,
    pub(crate) cursor: Position,
    pub(crate) cursor_visible: bool,
    pub(crate) dirty: Option<CellRect>,
    pub(crate) generation: u64,
    pub(crate) published: bool,
    /// Shared blink-phase toggle (RATATUI-00 §8). `true` renders blinking
    /// glyphs; `false` suppresses them (background only). Advanced only by
    /// [`RatatuiSurface::advance_blink_phase`] — never a wall clock.
    pub(crate) blink_phase: bool,
}

/// Shared retained cell surface written by [`RlvglBackend`] and painted by
/// [`RatatuiView`](crate::RatatuiView).
#[derive(Clone)]
pub struct RatatuiSurface(Rc<RefCell<SurfaceState>>);

impl RatatuiSurface {
    /// Current grid size in columns and rows.
    pub fn size(&self) -> Size {
        let state = self.0.borrow();
        Size::new(state.buffer.area.width, state.buffer.area.height)
    }

    /// Cell metrics used by the view.
    pub fn metrics(&self) -> CellMetrics {
        self.0.borrow().metrics
    }

    /// Clone the cell at `(x, y)`, or return `None` outside the grid.
    pub fn cell(&self, x: u16, y: u16) -> Option<Cell> {
        let state = self.0.borrow();
        state
            .published
            .then(|| state.buffer.cell((x, y)).cloned())
            .flatten()
    }

    /// Current cell-space dirty union.
    pub fn dirty_cells(&self) -> Option<CellRect> {
        let state = self.0.borrow();
        state.published.then_some(state.dirty).flatten()
    }

    /// Monotonic count of successful backend flushes.
    pub fn generation(&self) -> u64 {
        self.0.borrow().generation
    }

    pub(crate) fn borrow(&self) -> Ref<'_, SurfaceState> {
        self.0.borrow()
    }

    pub(crate) fn mark_clean(&self) {
        self.0.borrow_mut().dirty = None;
    }

    /// Flip the shared blink phase and mark the full surface dirty
    /// (RATATUI-00 §8).
    ///
    /// Caller-driven only — never a wall clock, `Instant`, or OS timer.
    /// Consuming applications wire this to whatever tick source they
    /// already own (the SCTD demo's Auto-tick cadence is the reference
    /// first consumer). The effect is observable through the existing
    /// [`generation`](Self::generation)/[`dirty_cells`](Self::dirty_cells)
    /// API once [`RlvglBackend::flush`] republishes the surface — no new
    /// accessor is added for this purpose. Mirrors the "mutate state,
    /// mark full-dirty, unpublish" pattern used by
    /// [`RlvglBackend::set_defaults`] and [`RlvglBackend::resize`].
    pub fn advance_blink_phase(&self) {
        let mut state = self.0.borrow_mut();
        state.blink_phase = !state.blink_phase;
        state.dirty = Some(full_rect(&state.buffer));
        state.published = false;
    }
}

/// Errors produced by [`RlvglBackend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RlvglBackendError {
    /// A grid or cell dimension was zero or its pixel product overflowed.
    InvalidDimensions,
    /// Ratatui attempted to draw outside the configured grid.
    OutOfBounds {
        /// Invalid column.
        x: u16,
        /// Invalid row.
        y: u16,
    },
}

impl fmt::Display for RlvglBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => formatter.write_str("invalid rlvgl terminal dimensions"),
            Self::OutOfBounds { x, y } => write!(formatter, "cell ({x}, {y}) is out of bounds"),
        }
    }
}

impl core::error::Error for RlvglBackendError {}

/// A Ratatui backend that retains cells for an rlvgl [`RatatuiView`](crate::RatatuiView).
pub struct RlvglBackend {
    surface: RatatuiSurface,
}

impl RlvglBackend {
    /// Create a backend and its shared view surface.
    pub fn new(
        columns: u16,
        rows: u16,
        metrics: CellMetrics,
    ) -> Result<(Self, RatatuiSurface), RlvglBackendError> {
        validate_dimensions(columns, rows, metrics)?;
        let full = CellRect {
            x: 0,
            y: 0,
            width: columns,
            height: rows,
        };
        let surface = RatatuiSurface(Rc::new(RefCell::new(SurfaceState {
            buffer: Buffer::empty(TuiRect::new(0, 0, columns, rows)),
            metrics,
            defaults: SurfaceDefaults::default(),
            cursor: Position::ORIGIN,
            cursor_visible: false,
            dirty: Some(full),
            generation: 0,
            published: true,
            blink_phase: true,
        })));
        Ok((
            Self {
                surface: surface.clone(),
            },
            surface,
        ))
    }

    /// Return a clone of the shared surface handle.
    pub fn surface(&self) -> RatatuiSurface {
        self.surface.clone()
    }

    /// Change the colors used for Ratatui `Color::Reset`.
    pub fn set_defaults(&self, defaults: SurfaceDefaults) {
        let mut state = self.surface.0.borrow_mut();
        state.defaults = defaults;
        state.dirty = Some(full_rect(&state.buffer));
        state.published = false;
    }

    /// Resize the retained grid and clear its content.
    pub fn resize(&self, columns: u16, rows: u16) -> Result<(), RlvglBackendError> {
        let metrics = self.surface.0.borrow().metrics;
        validate_dimensions(columns, rows, metrics)?;
        let mut state = self.surface.0.borrow_mut();
        state.buffer = Buffer::empty(TuiRect::new(0, 0, columns, rows));
        state.cursor.x = state.cursor.x.min(columns - 1);
        state.cursor.y = state.cursor.y.min(rows - 1);
        state.dirty = Some(full_rect(&state.buffer));
        state.published = false;
        Ok(())
    }

    fn clear_range(&self, start: usize, end: usize) {
        let mut state = self.surface.0.borrow_mut();
        let len = state.buffer.content.len();
        for cell in &mut state.buffer.content[start.min(len)..end.min(len)] {
            cell.reset();
        }
        state.dirty = Some(full_rect(&state.buffer));
        state.published = false;
    }
}

impl Backend for RlvglBackend {
    type Error = RlvglBackendError;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let size = self.size()?;
        let mut updates = Vec::new();
        for (x, y, cell) in content {
            if x >= size.width || y >= size.height {
                return Err(RlvglBackendError::OutOfBounds { x, y });
            }
            updates.push((x, y, cell.clone()));
        }
        let mut state = self.surface.0.borrow_mut();
        for (x, y, cell) in updates {
            let target = state
                .buffer
                .cell_mut((x, y))
                .expect("coordinates validated against the unchanged buffer");
            *target = cell.clone();
            let point = CellRect::point(x, y);
            state.dirty = Some(state.dirty.map_or(point, |dirty| dirty.union(point)));
        }
        state.published = false;
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        let mut state = self.surface.0.borrow_mut();
        let cursor = state.cursor;
        state.cursor_visible = false;
        mark_point(&mut state, cursor);
        state.published = false;
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        let mut state = self.surface.0.borrow_mut();
        let cursor = state.cursor;
        state.cursor_visible = true;
        mark_point(&mut state, cursor);
        state.published = false;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        Ok(self.surface.0.borrow().cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let position = position.into();
        let size = self.size()?;
        if position.x >= size.width || position.y >= size.height {
            return Err(RlvglBackendError::OutOfBounds {
                x: position.x,
                y: position.y,
            });
        }
        let mut state = self.surface.0.borrow_mut();
        let old = state.cursor;
        mark_point(&mut state, old);
        state.cursor = position;
        mark_point(&mut state, position);
        state.published = false;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        let len = self.surface.0.borrow().buffer.content.len();
        self.clear_range(0, len);
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        let state = self.surface.0.borrow();
        let width = state.buffer.area.width as usize;
        let height = state.buffer.area.height as usize;
        let cursor = state.cursor;
        drop(state);
        let index = cursor.y as usize * width + cursor.x as usize;
        match clear_type {
            ClearType::All => self.clear()?,
            ClearType::AfterCursor => self.clear_range(index, width * height),
            ClearType::BeforeCursor => self.clear_range(0, index + 1),
            ClearType::CurrentLine => {
                let start = cursor.y as usize * width;
                self.clear_range(start, start + width);
            }
            ClearType::UntilNewLine => {
                let end = (cursor.y as usize + 1) * width;
                self.clear_range(index, end);
            }
        }
        Ok(())
    }

    fn size(&self) -> Result<Size, Self::Error> {
        Ok(self.surface.size())
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        let state = self.surface.0.borrow();
        let columns_rows = Size::new(state.buffer.area.width, state.buffer.area.height);
        let width = columns_rows
            .width
            .checked_mul(state.metrics.width)
            .ok_or(RlvglBackendError::InvalidDimensions)?;
        let height = columns_rows
            .height
            .checked_mul(state.metrics.height)
            .ok_or(RlvglBackendError::InvalidDimensions)?;
        Ok(WindowSize {
            columns_rows,
            pixels: Size::new(width, height),
        })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let mut state = self.surface.0.borrow_mut();
        state.generation = state.generation.wrapping_add(1);
        state.published = true;
        Ok(())
    }
}

fn validate_dimensions(
    columns: u16,
    rows: u16,
    metrics: CellMetrics,
) -> Result<(), RlvglBackendError> {
    if columns == 0 || rows == 0 || metrics.width == 0 || metrics.height == 0 {
        return Err(RlvglBackendError::InvalidDimensions);
    }
    columns
        .checked_mul(metrics.width)
        .and_then(|_| rows.checked_mul(metrics.height))
        .ok_or(RlvglBackendError::InvalidDimensions)?;
    Ok(())
}

const fn full_rect(buffer: &Buffer) -> CellRect {
    CellRect {
        x: 0,
        y: 0,
        width: buffer.area.width,
        height: buffer.area.height,
    }
}

fn mark_point(state: &mut SurfaceState, position: Position) {
    let point = CellRect::point(position.x, position.y);
    state.dirty = Some(state.dirty.map_or(point, |dirty| dirty.union(point)));
}

#[cfg(test)]
mod tests {
    use ratatui_core::style::Color as TuiColor;

    use super::*;

    #[test]
    fn draw_retains_cells_and_dirty_union() {
        let (mut backend, surface) = RlvglBackend::new(4, 2, CellMetrics::font_6x10()).unwrap();
        surface.0.borrow_mut().dirty = None;
        let mut a = Cell::new("A");
        a.fg = TuiColor::Red;
        let b = Cell::new("B");
        backend.draw([(1, 0, &a), (3, 1, &b)].into_iter()).unwrap();
        backend.flush().unwrap();
        assert_eq!(surface.cell(1, 0).unwrap().symbol(), "A");
        assert_eq!(
            surface.dirty_cells(),
            Some(CellRect {
                x: 1,
                y: 0,
                width: 3,
                height: 2
            })
        );
    }

    #[test]
    fn flush_publishes_an_atomic_frame() {
        let (mut backend, surface) = RlvglBackend::new(2, 1, CellMetrics::font_6x10()).unwrap();
        surface.mark_clean();
        let cell = Cell::new("A");
        backend.draw([(0, 0, &cell)].into_iter()).unwrap();
        assert_eq!(surface.cell(0, 0), None);
        assert_eq!(surface.dirty_cells(), None);
        backend.flush().unwrap();
        assert_eq!(surface.cell(0, 0).unwrap().symbol(), "A");
        assert_eq!(surface.dirty_cells(), Some(CellRect::point(0, 0)));
    }

    #[test]
    fn cursor_changes_dirty_old_and_new_cells() {
        let (mut backend, surface) = RlvglBackend::new(3, 1, CellMetrics::font_6x10()).unwrap();
        surface.mark_clean();
        backend.show_cursor().unwrap();
        backend.set_cursor_position((2, 0)).unwrap();
        backend.flush().unwrap();
        assert_eq!(
            surface.dirty_cells(),
            Some(CellRect {
                x: 0,
                y: 0,
                width: 3,
                height: 1,
            })
        );
    }

    #[test]
    fn rejects_invalid_dimensions_and_draw_coordinates() {
        assert!(matches!(
            RlvglBackend::new(0, 2, CellMetrics::font_6x10()),
            Err(RlvglBackendError::InvalidDimensions)
        ));
        let (mut backend, _) = RlvglBackend::new(2, 2, CellMetrics::font_6x10()).unwrap();
        let cell = Cell::new("X");
        assert_eq!(
            backend.draw([(2, 0, &cell)].into_iter()),
            Err(RlvglBackendError::OutOfBounds { x: 2, y: 0 })
        );
    }

    #[test]
    fn clear_regions_follow_cursor() {
        let (mut backend, surface) = RlvglBackend::new(3, 2, CellMetrics::font_6x10()).unwrap();
        let cell = Cell::new("X");
        backend
            .draw(
                [
                    (0, 0, &cell),
                    (1, 0, &cell),
                    (2, 0, &cell),
                    (0, 1, &cell),
                    (1, 1, &cell),
                    (2, 1, &cell),
                ]
                .into_iter(),
            )
            .unwrap();
        backend.set_cursor_position((1, 0)).unwrap();
        backend.clear_region(ClearType::UntilNewLine).unwrap();
        backend.flush().unwrap();
        assert_eq!(surface.cell(0, 0).unwrap().symbol(), "X");
        assert_eq!(surface.cell(1, 0).unwrap().symbol(), " ");
        assert_eq!(surface.cell(2, 0).unwrap().symbol(), " ");
        assert_eq!(surface.cell(0, 1).unwrap().symbol(), "X");
    }

    #[test]
    fn advance_blink_phase_marks_full_dirty_and_unpublishes() {
        let (mut backend, surface) = RlvglBackend::new(3, 2, CellMetrics::font_6x10()).unwrap();
        backend.flush().unwrap();
        surface.mark_clean();
        let generation_before = surface.generation();

        surface.advance_blink_phase();

        // Mirrors set_defaults/resize: the mutation unpublishes immediately,
        // so it is not yet observable until the next flush.
        assert_eq!(surface.dirty_cells(), None);
        assert_eq!(surface.cell(0, 0), None);

        backend.flush().unwrap();
        assert_eq!(
            surface.dirty_cells(),
            Some(CellRect {
                x: 0,
                y: 0,
                width: 3,
                height: 2,
            })
        );
        assert!(surface.generation() > generation_before);
    }

    #[test]
    fn reports_character_and_pixel_size() {
        let (mut backend, _) = RlvglBackend::new(80, 24, CellMetrics::font_6x10()).unwrap();
        assert_eq!(
            backend.window_size().unwrap(),
            WindowSize {
                columns_rows: Size::new(80, 24),
                pixels: Size::new(960, 480)
            }
        );
    }
}
