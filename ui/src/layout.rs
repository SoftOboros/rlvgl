// SPDX-License-Identifier: MIT
//! Basic layout helpers for arranging [`Widget`] instances from
//! [`rlvgl_widgets`].
//!
//! Provides vertical and horizontal stacks, a simple grid, a box wrapper,
//! and a lightweight [`GridCalc`] geometry calculator.

use alloc::{boxed::Box, vec::Vec};
use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::container::Container;

/// Container that positions children vertically.
///
/// Accepts any [`Widget`] from [`rlvgl_widgets`] and arranges them
/// top-to-bottom.
pub struct VStack {
    bounds: Rect,
    spacing: i32,
    children: Vec<Box<dyn Widget>>,
    next_y: i32,
}

impl VStack {
    /// Create an empty vertical stack with the given width.
    pub fn new(width: i32) -> Self {
        Self {
            bounds: Rect {
                x: 0,
                y: 0,
                width,
                height: 0,
            },
            spacing: 0,
            children: Vec::new(),
            next_y: 0,
        }
    }

    /// Set the spacing between stacked children.
    pub fn spacing(mut self, spacing: i32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Add a child of the given height, created by the supplied builder.
    pub fn child<W, F>(mut self, height: i32, builder: F) -> Self
    where
        W: Widget + 'static,
        F: FnOnce(Rect) -> W,
    {
        let rect = Rect {
            x: 0,
            y: self.next_y,
            width: self.bounds.width,
            height,
        };
        self.next_y += height + self.spacing;
        self.bounds.height = self.next_y - self.spacing;
        self.children.push(Box::new(builder(rect)));
        self
    }
}

impl Widget for VStack {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        for child in &self.children {
            child.draw(renderer);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        for child in &mut self.children {
            if child.handle_event(event) {
                return true;
            }
        }
        false
    }
}

/// Container that positions children horizontally.
///
/// Like [`VStack`], this operates on [`Widget`] instances from
/// [`rlvgl_widgets`].
pub struct HStack {
    bounds: Rect,
    spacing: i32,
    children: Vec<Box<dyn Widget>>,
    next_x: i32,
}

impl HStack {
    /// Create an empty horizontal stack with the given height.
    pub fn new(height: i32) -> Self {
        Self {
            bounds: Rect {
                x: 0,
                y: 0,
                width: 0,
                height,
            },
            spacing: 0,
            children: Vec::new(),
            next_x: 0,
        }
    }

    /// Set the spacing between stacked children.
    pub fn spacing(mut self, spacing: i32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Add a child of the given width, created by the supplied builder.
    pub fn child<W, F>(mut self, width: i32, builder: F) -> Self
    where
        W: Widget + 'static,
        F: FnOnce(Rect) -> W,
    {
        let rect = Rect {
            x: self.next_x,
            y: 0,
            width,
            height: self.bounds.height,
        };
        self.next_x += width + self.spacing;
        self.bounds.width = self.next_x - self.spacing;
        self.children.push(Box::new(builder(rect)));
        self
    }
}

impl Widget for HStack {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        for child in &self.children {
            child.draw(renderer);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        for child in &mut self.children {
            if child.handle_event(event) {
                return true;
            }
        }
        false
    }
}

/// Simple grid container placing widgets in fixed-size cells.
pub struct Grid {
    bounds: Rect,
    cols: i32,
    cell_w: i32,
    cell_h: i32,
    spacing: i32,
    children: Vec<Box<dyn Widget>>,
    next: i32,
}

impl Grid {
    /// Create a new grid with the given cell size and column count.
    pub fn new(cols: i32, cell_w: i32, cell_h: i32) -> Self {
        Self {
            bounds: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            cols,
            cell_w,
            cell_h,
            spacing: 0,
            children: Vec::new(),
            next: 0,
        }
    }

    /// Set the spacing between grid cells.
    pub fn spacing(mut self, spacing: i32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Add a child placed in the next grid cell.
    pub fn child<W, F>(mut self, builder: F) -> Self
    where
        W: Widget + 'static,
        F: FnOnce(Rect) -> W,
    {
        let col = self.next % self.cols;
        let row = self.next / self.cols;
        let x = col * (self.cell_w + self.spacing);
        let y = row * (self.cell_h + self.spacing);
        let rect = Rect {
            x,
            y,
            width: self.cell_w,
            height: self.cell_h,
        };
        self.children.push(Box::new(builder(rect)));
        self.next += 1;
        let w = x + self.cell_w;
        let h = y + self.cell_h;
        if w > self.bounds.width {
            self.bounds.width = w;
        }
        if h > self.bounds.height {
            self.bounds.height = h;
        }
        self
    }
}

impl Widget for Grid {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        for child in &self.children {
            child.draw(renderer);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        for child in &mut self.children {
            if child.handle_event(event) {
                return true;
            }
        }
        false
    }
}

/// Generic container box that wraps the base `Container` widget.
pub struct BoxLayout {
    inner: Container,
}

impl BoxLayout {
    /// Create a new box with the provided bounds.
    pub fn new(bounds: Rect) -> Self {
        Self {
            inner: Container::new(bounds),
        }
    }

    /// Mutable access to the inner style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for BoxLayout {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        self.inner.handle_event(event)
    }
}

/// Pure-geometry grid calculator for manual widget layouts.
///
/// Unlike [`Grid`], this does not own widgets — it only computes
/// cell [`Rect`]s from `(row, col)` indices, making it suitable for
/// custom widgets that do their own drawing and hit-testing.
///
/// ```
/// # use rlvgl_core::widget::Rect;
/// # use rlvgl_ui::layout::GridCalc;
/// let g = GridCalc::new(10, 20, 2, 100, 40).gap(4, 2);
/// let r = g.cell(1, 0);
/// assert_eq!(r, Rect { x: 10, y: 62, width: 100, height: 40 });
/// ```
pub struct GridCalc {
    /// Top-left origin X.
    pub x: i32,
    /// Top-left origin Y.
    pub y: i32,
    /// Number of columns.
    pub cols: usize,
    /// Width of each cell.
    pub col_w: i32,
    /// Height of each cell.
    pub row_h: i32,
    /// Horizontal gap between columns.
    pub col_gap: i32,
    /// Vertical gap between rows.
    pub row_gap: i32,
}

impl GridCalc {
    /// Create a grid calculator with the given origin, column count, and cell size.
    pub const fn new(x: i32, y: i32, cols: usize, col_w: i32, row_h: i32) -> Self {
        Self {
            x,
            y,
            cols,
            col_w,
            row_h,
            col_gap: 0,
            row_gap: 0,
        }
    }

    /// Set inter-cell gaps.
    pub const fn gap(mut self, col_gap: i32, row_gap: i32) -> Self {
        self.col_gap = col_gap;
        self.row_gap = row_gap;
        self
    }

    /// Return the `Rect` for the cell at `(row, col)`.
    pub const fn cell(&self, row: usize, col: usize) -> Rect {
        Rect {
            x: self.x + col as i32 * (self.col_w + self.col_gap),
            y: self.y + row as i32 * (self.row_h + self.row_gap),
            width: self.col_w,
            height: self.row_h,
        }
    }

    /// Return a `Rect` spanning all columns for the given `row`.
    pub const fn row_span(&self, row: usize) -> Rect {
        let total_w = if self.cols == 0 {
            0
        } else {
            self.cols as i32 * self.col_w + (self.cols as i32 - 1) * self.col_gap
        };
        Rect {
            x: self.x,
            y: self.y + row as i32 * (self.row_h + self.row_gap),
            width: total_w,
            height: self.row_h,
        }
    }

    /// Total width of the grid (all columns + gaps).
    pub const fn total_width(&self) -> i32 {
        if self.cols == 0 {
            0
        } else {
            self.cols as i32 * self.col_w + (self.cols as i32 - 1) * self.col_gap
        }
    }

    /// Total height of the grid for the given number of rows.
    pub const fn total_height(&self, rows: usize) -> i32 {
        if rows == 0 {
            0
        } else {
            rows as i32 * self.row_h + (rows as i32 - 1) * self.row_gap
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_widgets::label::Label;

    #[test]
    fn vstack_stacks_vertically() {
        let stack = VStack::new(20)
            .spacing(2)
            .child(10, |r| Label::new("a", r))
            .child(10, |r| Label::new("b", r));
        assert_eq!(stack.bounds().height, 22);
    }

    #[test]
    fn hstack_stacks_horizontally() {
        let stack = HStack::new(10)
            .spacing(1)
            .child(5, |r| Label::new("a", r))
            .child(5, |r| Label::new("b", r));
        assert_eq!(stack.bounds().width, 11);
    }

    #[test]
    fn grid_places_cells() {
        let grid = Grid::new(2, 5, 5)
            .spacing(1)
            .child(|r| Label::new("a", r))
            .child(|r| Label::new("b", r))
            .child(|r| Label::new("c", r));
        assert_eq!(grid.bounds().height, 11);
        assert_eq!(grid.bounds().width, 11);
    }

    #[test]
    fn grid_calc_cell() {
        let g = GridCalc::new(10, 20, 2, 100, 40).gap(4, 2);
        let r = g.cell(0, 0);
        assert_eq!(
            r,
            Rect {
                x: 10,
                y: 20,
                width: 100,
                height: 40
            }
        );
        let r = g.cell(1, 1);
        assert_eq!(
            r,
            Rect {
                x: 114,
                y: 62,
                width: 100,
                height: 40
            }
        );
    }

    #[test]
    fn grid_calc_row_span() {
        let g = GridCalc::new(0, 0, 3, 50, 30).gap(10, 5);
        let r = g.row_span(0);
        assert_eq!(
            r,
            Rect {
                x: 0,
                y: 0,
                width: 170,
                height: 30
            }
        );
        assert_eq!(g.total_width(), 170);
        assert_eq!(g.total_height(2), 65);
    }
}
