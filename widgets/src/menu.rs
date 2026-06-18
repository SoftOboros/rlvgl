//! Multi-page hierarchical navigation widget (LPAR-13 §5.E).
//!
//! A [`Menu`] owns a set of [`MenuPage`]s, each containing a list of
//! [`MenuItem`] rows (labels, sub-page links, and separators).  Navigation
//! pushes child pages onto an internal stack; [`back`](Menu::back) pops it.
//!
//! # Draw layout
//!
//! - **Header bar** — drawn at the top (or bottom for `BottomFixed`) with
//!   the current page title and an optional back-button.
//! - **Item list** — the remaining height holds scrollable item rows.
//!
//! In v1 item layout uses static row heights (`16 px`, matching
//! [`crate::list::List`]).  Full LPAR-10 flex layout is deferred-Coupled.
//!
//! # Key navigation (LPAR-12/LPAR-13 §5.J)
//!
//! Wire `ObjectEvent::Key` to the imperative helpers:
//!
//! ```ignore
//! menu.navigate_next();      // ArrowDown
//! menu.navigate_prev();      // ArrowUp
//! menu.activate_selected();  // Enter — drills into sub-page or marks item
//! menu.back();               // Escape / back button
//! ```

use alloc::{string::String, vec::Vec};
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Row height for items in pixels (mirrors [`crate::list::List`]).
const ROW_HEIGHT: i32 = 16;
/// Height of the header bar in pixels.
const HEADER_HEIGHT: i32 = 22;
/// Height of a separator row in pixels.
const SEPARATOR_HEIGHT: i32 = 4;

// ── Public types ──────────────────────────────────────────────────────────────

/// Opaque identifier for a [`MenuPage`] within a [`Menu`].
///
/// Assigned sequentially by [`Menu::add_page`].
/// `PAGE_NONE` is the sentinel for "no page".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuPageId(pub u16);

/// Sentinel [`MenuPageId`] meaning "no page".
pub const PAGE_NONE: MenuPageId = MenuPageId(u16::MAX);

/// Opaque identifier for a [`MenuItem`] within a page.
///
/// Assigned sequentially by [`Menu::add_item`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuItemId(pub u16);

/// A single row inside a [`MenuPage`].
///
/// Registration policy: Specification Required (LPAR-13 §7).
#[derive(Debug, Clone)]
pub enum MenuItem {
    /// Plain text label.
    Label(String),
    /// Text label that navigates to `target` page when activated.
    SubPage {
        /// Displayed text for this item.
        label: String,
        /// Page to push onto the stack when activated.
        target: MenuPageId,
    },
    /// Horizontal rule / divider between groups of items.
    Separator,
}

impl MenuItem {
    /// Return `true` when this item can be navigated to / highlighted.
    fn is_navigable(&self) -> bool {
        !matches!(self, MenuItem::Separator)
    }

    /// Row height for this item variant.
    fn row_height(&self) -> i32 {
        match self {
            MenuItem::Separator => SEPARATOR_HEIGHT,
            _ => ROW_HEIGHT,
        }
    }
}

/// Position of the header bar within the [`Menu`] bounds.
///
/// Registration policy: Specification Required (LPAR-13 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHeaderMode {
    /// Header bar pinned to the top of the widget (default).
    TopFixed,
    /// Header bar at the top but scrolls with content (deferred-Safe in v1;
    /// behaves as `TopFixed` pending LPAR-05 ObjectNode integration).
    TopUnfixed,
    /// Header bar pinned to the bottom of the widget.
    BottomFixed,
}

// ── Internal page structure ───────────────────────────────────────────────────

struct MenuPage {
    title: String,
    items: Vec<MenuItem>,
}

// ── Menu ──────────────────────────────────────────────────────────────────────

/// Multi-page hierarchical navigation surface (LPAR-13 §5.E).
///
/// Stores all page data and item strings as owned values.
pub struct Menu {
    bounds: Rect,
    pages: Vec<MenuPage>,
    /// Stack of page ids; `stack.last()` is the currently displayed page.
    stack: Vec<MenuPageId>,
    /// Root page (shown when nothing is on the stack).
    root_page: MenuPageId,
    /// Index within the current page's navigable items (None = no highlight).
    highlight: Option<usize>,
    header_mode: MenuHeaderMode,
    show_root_back_button: bool,
    /// Background / border style for the full widget (`Part::MAIN`).
    pub style: Style,
    /// Style applied to item rows (`Part::ITEMS`).
    pub item_style: Style,
    /// Highlight color for the focused item (`Part::SELECTED`).
    pub selected_color: Color,
    /// Background color for the header bar.
    pub header_color: Color,
    /// Color for all text labels.
    pub text_color: Color,
    /// Color for separator rules.
    pub separator_color: Color,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Menu {
    /// Create a [`Menu`] widget occupying `bounds` with no pages.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            pages: Vec::new(),
            stack: Vec::new(),
            root_page: PAGE_NONE,
            highlight: None,
            header_mode: MenuHeaderMode::TopFixed,
            show_root_back_button: false,
            style: Style::default(),
            item_style: Style {
                bg_color: Color(245, 245, 245, 255),
                ..Style::default()
            },
            selected_color: Color(160, 200, 240, 255),
            header_color: Color(80, 80, 180, 255),
            text_color: Color(20, 20, 20, 255),
            separator_color: Color(180, 180, 180, 255),
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    // ── Page management ───────────────────────────────────────────────────

    /// Create a new page with the given `title` and return its id.
    pub fn add_page(&mut self, title: &str) -> MenuPageId {
        let id = MenuPageId(self.pages.len() as u16);
        self.pages.push(MenuPage {
            title: title.into(),
            items: Vec::new(),
        });
        id
    }

    /// Set the root (initial) page displayed when the stack is empty.
    pub fn set_root_page(&mut self, id: MenuPageId) {
        self.root_page = id;
        // Reset stack so the root is visible immediately
        self.stack.clear();
        self.highlight = None;
    }

    /// Append `item` to `page` and return its id within that page.
    ///
    /// The item string data is owned by the widget.
    pub fn add_item(&mut self, page: MenuPageId, item: MenuItem) -> MenuItemId {
        let idx = page.0 as usize;
        if idx >= self.pages.len() {
            return MenuItemId(u16::MAX);
        }
        let item_id = MenuItemId(self.pages[idx].items.len() as u16);
        self.pages[idx].items.push(item);
        item_id
    }

    /// Navigate directly to `id`, pushing it onto the stack.
    ///
    /// If `id` is already in the stack, pops back to that position.
    pub fn set_page(&mut self, id: MenuPageId) {
        if let Some(pos) = self.stack.iter().position(|&p| p == id) {
            self.stack.truncate(pos + 1);
        } else {
            self.stack.push(id);
        }
        self.highlight = None;
    }

    /// Pop the current page from the stack.
    ///
    /// No-op when already at the root page (i.e. `active_page() == root_page`
    /// and the stack is empty or has the root page at position 0).
    pub fn back(&mut self) {
        // If the stack is empty the root page is showing — nothing to pop.
        if self.stack.is_empty() {
            return;
        }
        // If the visible page is the designated root page, only allow back when
        // the root back-button is explicitly enabled.
        let top = *self.stack.last().unwrap();
        if top == self.root_page && !self.show_root_back_button {
            return;
        }
        self.stack.pop();
        self.highlight = None;
    }

    /// Return the id of the currently displayed page.
    pub fn active_page(&self) -> MenuPageId {
        self.stack.last().copied().unwrap_or(self.root_page)
    }

    // ── Header configuration ──────────────────────────────────────────────

    /// Set the position of the header bar.
    pub fn set_header_mode(&mut self, mode: MenuHeaderMode) {
        self.header_mode = mode;
    }

    /// Return the current header mode.
    pub fn header_mode(&self) -> MenuHeaderMode {
        self.header_mode
    }

    /// Enable or disable the back button when on the root page.
    pub fn set_root_back_button(&mut self, enable: bool) {
        self.show_root_back_button = enable;
    }

    /// Return whether the root back button is enabled.
    pub fn root_back_button(&self) -> bool {
        self.show_root_back_button
    }

    // ── Key navigation helpers (LPAR-13 §5.J) ────────────────────────────

    /// Move the highlight to the next navigable item on the current page,
    /// wrapping at the end.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowDown)`.
    pub fn navigate_next(&mut self) {
        let items = self.current_items();
        if items.is_empty() {
            return;
        }
        let navigable: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, it)| it.is_navigable())
            .map(|(i, _)| i)
            .collect();
        if navigable.is_empty() {
            return;
        }
        let next = match self.highlight {
            None => navigable[0],
            Some(cur) => {
                let pos = navigable.iter().position(|&i| i == cur);
                match pos {
                    Some(p) => navigable[(p + 1) % navigable.len()],
                    None => navigable[0],
                }
            }
        };
        self.highlight = Some(next);
    }

    /// Move the highlight to the previous navigable item on the current page,
    /// wrapping at the start.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowUp)`.
    pub fn navigate_prev(&mut self) {
        let items = self.current_items();
        if items.is_empty() {
            return;
        }
        let navigable: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, it)| it.is_navigable())
            .map(|(i, _)| i)
            .collect();
        if navigable.is_empty() {
            return;
        }
        let prev = match self.highlight {
            None => *navigable.last().unwrap(),
            Some(cur) => {
                let pos = navigable.iter().position(|&i| i == cur);
                match pos {
                    Some(p) => navigable[(p + navigable.len() - 1) % navigable.len()],
                    None => *navigable.last().unwrap(),
                }
            }
        };
        self.highlight = Some(prev);
    }

    /// Activate the currently highlighted item.
    ///
    /// - [`MenuItem::SubPage`] — pushes the target page onto the stack.
    /// - [`MenuItem::Label`] — no structural change (caller polls via `active_page` etc.).
    /// - [`MenuItem::Separator`] — not reachable by navigation.
    ///
    /// Wire to `ObjectEvent::Key(Key::Enter)`.
    pub fn activate_selected(&mut self) {
        let Some(hi) = self.highlight else { return };
        // Inspect the current page's items by index.  We need to clone the
        // target page id if it's a SubPage to avoid holding an immutable borrow
        // while calling set_page (which takes &mut self).
        let maybe_target: Option<MenuPageId> = self
            .current_page_ref()
            .and_then(|p| p.items.get(hi))
            .and_then(|item| {
                if let MenuItem::SubPage { target, .. } = item {
                    Some(*target)
                } else {
                    None
                }
            });

        if let Some(target) = maybe_target {
            self.stack.push(target);
            self.highlight = None;
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    /// Return the page index for `id`, or `None`.
    fn page_index(&self, id: MenuPageId) -> Option<usize> {
        let idx = id.0 as usize;
        if idx < self.pages.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Return a reference to the current page, if any.
    fn current_page_ref(&self) -> Option<&MenuPage> {
        let id = self.active_page();
        self.page_index(id).map(|i| &self.pages[i])
    }

    /// Collect a slice of items on the current page (zero-copy borrow).
    fn current_items(&self) -> &[MenuItem] {
        self.current_page_ref()
            .map(|p| p.items.as_slice())
            .unwrap_or(&[])
    }

    /// Geometry: header rect based on header mode.
    fn header_rect(&self) -> Rect {
        match self.header_mode {
            MenuHeaderMode::BottomFixed => Rect {
                x: self.bounds.x,
                y: self.bounds.y + self.bounds.height - HEADER_HEIGHT,
                width: self.bounds.width,
                height: HEADER_HEIGHT,
            },
            _ => Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: self.bounds.width,
                height: HEADER_HEIGHT.min(self.bounds.height),
            },
        }
    }

    /// Geometry: item list rect (complement of header).
    fn list_rect(&self) -> Rect {
        let hdr = self.header_rect();
        match self.header_mode {
            MenuHeaderMode::BottomFixed => Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: self.bounds.width,
                height: (self.bounds.height - HEADER_HEIGHT).max(0),
            },
            _ => Rect {
                x: self.bounds.x,
                y: hdr.y + hdr.height,
                width: self.bounds.width,
                height: (self.bounds.height - hdr.height).max(0),
            },
        }
    }
}

impl Widget for Menu {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        Some(&mut self.font)
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let a = self.style.alpha;
        let font = self.font.resolve();

        // ── Widget background (Part::MAIN) ────────────────────────────────
        draw_widget_bg(renderer, self.bounds, &self.style);

        // ── Header bar ────────────────────────────────────────────────────
        let hdr = self.header_rect();
        if hdr.height > 0 {
            renderer.fill_rect(hdr, self.header_color.with_alpha(a));

            // Show back button when we can navigate back.
            let can_go_back = !self.stack.is_empty()
                && (*self.stack.last().unwrap() != self.root_page || self.show_root_back_button);
            let is_at_root = !can_go_back;
            let title_x_offset = if !is_at_root {
                let back_shaped = shape_text_ltr(font, "<", (hdr.x + 4, hdr.y + 15), 0);
                renderer.draw_text_shaped(
                    &back_shaped,
                    (0, 0),
                    Color(255, 255, 255, 255).with_alpha(a),
                );
                16
            } else {
                4
            };

            // Page title
            if let Some(page) = self.current_page_ref() {
                let shaped =
                    shape_text_ltr(font, &page.title, (hdr.x + title_x_offset, hdr.y + 15), 0);
                renderer.draw_text_shaped(&shaped, (0, 0), Color(255, 255, 255, 255).with_alpha(a));
            }
        }

        // ── Item list (Part::ITEMS / Part::SELECTED) ──────────────────────
        let lr = self.list_rect();
        if lr.height <= 0 {
            return;
        }
        draw_widget_bg(renderer, lr, &self.item_style);

        let item_a = self.item_style.alpha;
        let items = self.current_items();
        let mut y = lr.y;
        for (i, item) in items.iter().enumerate() {
            if y >= lr.y + lr.height {
                break;
            }
            let rh = item.row_height();
            let row_rect = Rect {
                x: lr.x,
                y,
                width: lr.width,
                height: rh.min(lr.y + lr.height - y).max(0),
            };

            match item {
                MenuItem::Separator => {
                    // Draw a horizontal line
                    renderer.fill_rect(
                        Rect {
                            x: lr.x + 4,
                            y: y + rh / 2,
                            width: lr.width - 8,
                            height: 1,
                        },
                        self.separator_color.with_alpha(item_a),
                    );
                }
                MenuItem::Label(text) | MenuItem::SubPage { label: text, .. } => {
                    // Selected / highlighted item background
                    if self.highlight == Some(i) {
                        renderer.fill_rect(row_rect, self.selected_color.with_alpha(item_a));
                    }

                    // Item text
                    let baseline = y + rh - 2;
                    let shaped = shape_text_ltr(font, text, (lr.x + 6, baseline), 0);
                    renderer.draw_text_shaped(&shaped, (0, 0), self.text_color.with_alpha(item_a));

                    // Sub-page arrow indicator
                    if matches!(item, MenuItem::SubPage { .. }) {
                        let arrow_x = lr.x + lr.width - 10;
                        let arrow_y = y + rh / 2 - 2;
                        // Simple ">" drawn as 3 fill_rect calls
                        for dy in 0..3i32 {
                            let x_off = if dy == 1 { 3 } else { 0 };
                            renderer.fill_rect(
                                Rect {
                                    x: arrow_x + x_off,
                                    y: arrow_y + dy,
                                    width: 1,
                                    height: 1,
                                },
                                self.text_color.with_alpha(item_a),
                            );
                        }
                    }
                }
            }

            y += rh;
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        let Event::PressRelease { x, y } = event else {
            return false;
        };

        // Click on header back-button area
        let hdr = self.header_rect();
        if *x >= hdr.x && *x < hdr.x + 16 && *y >= hdr.y && *y < hdr.y + hdr.height {
            let can_go_back = !self.stack.is_empty()
                && (*self.stack.last().unwrap() != self.root_page || self.show_root_back_button);
            if can_go_back {
                self.back();
                return true;
            }
        }

        // Click on an item row
        let lr = self.list_rect();
        if *x < lr.x || *x >= lr.x + lr.width || *y < lr.y || *y >= lr.y + lr.height {
            return false;
        }

        let mut row_y = lr.y;
        let items = self.current_items();
        for (i, item) in items.iter().enumerate() {
            let rh = item.row_height();
            if *y >= row_y && *y < row_y + rh {
                if item.is_navigable() {
                    self.highlight = Some(i);
                    self.activate_selected();
                    return true;
                }
                return false;
            }
            row_y += rh;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    struct NullRenderer;
    impl rlvgl_core::renderer::Renderer for NullRenderer {
        fn fill_rect(&mut self, _: Rect, _: Color) {}
        fn draw_text(&mut self, _: (i32, i32), _: &str, _: Color) {}
    }

    #[test]
    fn new_menu_has_no_pages() {
        let m = Menu::new(rect(0, 0, 200, 300));
        assert_eq!(m.active_page(), PAGE_NONE);
    }

    #[test]
    fn add_page_and_set_root() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let root = m.add_page("Root");
        m.set_root_page(root);
        assert_eq!(m.active_page(), root);
    }

    #[test]
    fn items_are_owned_strings() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let p = m.add_page("P");
        {
            let label = alloc::format!("owned {}", 42);
            m.add_item(p, MenuItem::Label(label));
        }
        // Data still accessible after label string dropped
        m.set_root_page(p);
        let items = m.current_items();
        assert_eq!(items.len(), 1);
        if let MenuItem::Label(ref s) = items[0] {
            assert_eq!(s, "owned 42");
        } else {
            panic!("expected Label");
        }
    }

    #[test]
    fn navigate_next_and_prev_move_highlight() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let p = m.add_page("P");
        m.add_item(p, MenuItem::Label("A".into()));
        m.add_item(p, MenuItem::Separator);
        m.add_item(p, MenuItem::Label("B".into()));
        m.set_root_page(p);

        m.navigate_next();
        assert_eq!(m.highlight, Some(0)); // A

        m.navigate_next();
        assert_eq!(m.highlight, Some(2)); // B (separator skipped)

        m.navigate_next(); // wrap → A
        assert_eq!(m.highlight, Some(0));

        m.navigate_prev(); // B
        assert_eq!(m.highlight, Some(2));
    }

    #[test]
    fn activate_sub_page_pushes_stack() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let root = m.add_page("Root");
        let child = m.add_page("Child");
        m.add_item(
            root,
            MenuItem::SubPage {
                label: "Go to child".into(),
                target: child,
            },
        );
        m.set_root_page(root);

        m.navigate_next();
        m.activate_selected();
        assert_eq!(m.active_page(), child);
    }

    #[test]
    fn back_pops_to_parent() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let root = m.add_page("Root");
        let child = m.add_page("Child");
        m.add_item(
            root,
            MenuItem::SubPage {
                label: "Child".into(),
                target: child,
            },
        );
        m.set_root_page(root);
        m.set_page(child);
        assert_eq!(m.active_page(), child);

        m.back();
        // After back the stack should be empty or at root
        // stack was [child], back pops → stack is empty → active is root_page
        // Actually set_page pushes: stack = [child]; back pops → stack = []
        // active_page returns stack.last() or root_page → root
        assert_eq!(m.active_page(), root);
    }

    #[test]
    fn back_is_noop_at_root() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let root = m.add_page("Root");
        m.set_root_page(root);
        m.back(); // should not panic
        assert_eq!(m.active_page(), root);
    }

    #[test]
    fn set_bounds_adopted() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        m.set_bounds(rect(5, 10, 250, 350));
        assert_eq!(m.bounds(), rect(5, 10, 250, 350));
    }

    #[test]
    fn draw_does_not_panic() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let p = m.add_page("Main");
        m.add_item(p, MenuItem::Label("Item 1".into()));
        m.add_item(p, MenuItem::Separator);
        let child = m.add_page("Settings");
        m.add_item(
            p,
            MenuItem::SubPage {
                label: "Settings".into(),
                target: child,
            },
        );
        m.set_root_page(p);
        let mut r = NullRenderer;
        m.draw(&mut r);
    }

    #[test]
    fn header_mode_bottom_fixed() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        m.set_header_mode(MenuHeaderMode::BottomFixed);
        assert_eq!(m.header_mode(), MenuHeaderMode::BottomFixed);
        let hdr = m.header_rect();
        // Header should be at the bottom
        assert_eq!(hdr.y + hdr.height, m.bounds.y + m.bounds.height);
    }

    #[test]
    fn set_page_navigates_and_back_returns() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let root = m.add_page("Root");
        let child = m.add_page("Child");
        let grandchild = m.add_page("Grandchild");
        m.set_root_page(root);
        m.set_page(child);
        m.set_page(grandchild);
        assert_eq!(m.active_page(), grandchild);
        m.back();
        assert_eq!(m.active_page(), child);
        m.back();
        assert_eq!(m.active_page(), root);
    }

    #[test]
    fn add_item_oob_page_returns_sentinel() {
        let mut m = Menu::new(rect(0, 0, 200, 300));
        let id = m.add_item(PAGE_NONE, MenuItem::Label("X".into()));
        assert_eq!(id.0, u16::MAX);
    }
}
