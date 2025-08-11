// SPDX-License-Identifier: MIT
//! Basic layout helpers for arranging widgets.
//!
//! Provides vertical and horizontal stacks, a simple grid, and a box wrapper.

use alloc::{boxed::Box, vec::Vec};
use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::container::Container;

/// Container that positions children vertically.
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
}
