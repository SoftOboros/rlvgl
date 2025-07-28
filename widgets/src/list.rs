use alloc::{string::String, vec::Vec};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Scrollable list of selectable text items.
pub struct List {
    bounds: Rect,
    pub style: Style,
    pub text_color: Color,
    items: Vec<String>,
    selected: Option<usize>,
}

impl List {
    /// Create an empty list widget.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            style: Style::default(),
            text_color: Color(0, 0, 0),
            items: Vec::new(),
            selected: None,
        }
    }

    /// Append an item to the end of the list.
    pub fn add_item(&mut self, text: impl Into<String>) {
        self.items.push(text.into());
    }

    /// Return a slice of all list items.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Index of the currently selected item, if any.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Translate a y coordinate into a list index.
    fn index_at(&self, y: i32) -> Option<usize> {
        let row_height = 16;
        if y < self.bounds.y || y >= self.bounds.y + self.bounds.height {
            return None;
        }
        let idx = (y - self.bounds.y) / row_height;
        if idx < 0 {
            return None;
        }
        let idx = idx as usize;
        if idx < self.items.len() {
            Some(idx)
        } else {
            None
        }
    }
}

impl Widget for List {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        renderer.fill_rect(self.bounds, self.style.bg_color);
        let row_height = 16;
        for (i, item) in self.items.iter().enumerate() {
            let y = self.bounds.y + (i as i32 * row_height);
            let pos = (self.bounds.x + 2, y);
            let color = if self.selected == Some(i) {
                self.style.border_color
            } else {
                self.text_color
            };
            renderer.draw_text(pos, item, color);
        }
    }

    /// Select an item when the pointer is released over it.
    fn handle_event(&mut self, event: &Event) -> bool {
        let Event::PointerUp { x, y } = event else {
            return false;
        };

        if *x < self.bounds.x || *x >= self.bounds.x + self.bounds.width {
            return false;
        }

        let Some(idx) = self.index_at(*y) else {
            return false;
        };

        self.selected = Some(idx);
        true
    }
}
