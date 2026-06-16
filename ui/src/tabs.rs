// SPDX-License-Identifier: MIT
//! Tabs component backed by the LVGL-parity tabview widget.
//!
//! [`Tabs`] provides the app-facing name for
//! [`rlvgl_widgets::tabview::Tabview`] while preserving explicit tab IDs and
//! content bounds.

use alloc::boxed::Box;
use rlvgl_core::{
    event::Event,
    font::{FontMetrics, WidgetFont},
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::tabview::Tabview;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

pub use rlvgl_widgets::tabview::{TabBarPos, TabId};

/// Tab bar plus content-pane navigation component.
pub struct Tabs {
    inner: Tabview,
    on_change: Option<Box<dyn FnMut(TabId)>>,
}

impl Tabs {
    /// Sentinel tab ID meaning no active tab.
    pub const TAB_NONE: TabId = Tabview::TAB_NONE;

    /// Create a tab container with the given tab bar position.
    pub fn new(bounds: Rect, bar_pos: TabBarPos) -> Self {
        Self {
            inner: Tabview::new(bounds, bar_pos),
            on_change: None,
        }
    }

    /// Append a tab and return the widget.
    pub fn tab(mut self, name: &str) -> Self {
        self.inner.add_tab(name);
        self
    }

    /// Append a tab and return its ID.
    pub fn add_tab(&mut self, name: &str) -> TabId {
        self.inner.add_tab(name)
    }

    /// Rename the tab with the given ID.
    pub fn rename_tab(&mut self, id: TabId, name: &str) {
        self.inner.rename_tab(id, name);
    }

    /// Register a callback fired when the active tab changes through this wrapper.
    pub fn on_change<F: FnMut(TabId) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Make the tab with the given ID active and return the widget.
    pub fn with_active_tab(mut self, id: TabId) -> Self {
        self.set_active(id);
        self
    }

    /// Make the tab with the given ID active.
    pub fn set_active(&mut self, id: TabId) {
        let before = self.inner.active_tab();
        self.inner.set_active(id);
        self.emit_if_changed(before);
    }

    /// Return the currently active tab.
    pub fn active_tab(&self) -> TabId {
        self.inner.active_tab()
    }

    /// Return the number of registered tabs.
    pub fn tab_count(&self) -> usize {
        self.inner.tab_count()
    }

    /// Return the content-area rect for the given tab.
    pub fn tab_content_bounds(&self, id: TabId) -> Rect {
        self.inner.tab_content_bounds(id)
    }

    /// Return the tab bar position.
    pub fn bar_pos(&self) -> TabBarPos {
        self.inner.bar_pos()
    }

    /// Set the tab bar position.
    pub fn set_bar_pos(&mut self, pos: TabBarPos) {
        self.inner.set_bar_pos(pos);
    }

    /// Advance to the next tab.
    pub fn navigate_next_tab(&mut self) {
        let before = self.inner.active_tab();
        self.inner.navigate_next_tab();
        self.emit_if_changed(before);
    }

    /// Move to the previous tab.
    pub fn navigate_prev_tab(&mut self) {
        let before = self.inner.active_tab();
        self.inner.navigate_prev_tab();
        self.emit_if_changed(before);
    }

    /// Set inactive tab button color and return the widget.
    pub fn tab_color(mut self, color: Color) -> Self {
        self.inner.tab_color = color;
        self
    }

    /// Set active tab button color and return the widget.
    pub fn active_tab_color(mut self, color: Color) -> Self {
        self.inner.active_tab_color = color;
        self
    }

    /// Set inactive tab text color and return the widget.
    pub fn tab_text_color(mut self, color: Color) -> Self {
        self.inner.tab_text_color = color;
        self
    }

    /// Set active tab text color and return the widget.
    pub fn active_tab_text_color(mut self, color: Color) -> Self {
        self.inner.active_tab_text_color = color;
        self
    }

    /// Assign the font used to render this tab bar.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.inner.set_font(font);
    }

    /// Apply themed tab container, active tab, and inactive tab colors.
    pub fn themed(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        let colors = theme.scheme(scheme);
        self.inner.style = resolved.style;
        self.inner.active_tab_color = resolved.style.bg_color;
        self.inner.active_tab_text_color = resolved.text_color;
        self.inner.tab_color = colors.subtle;
        self.inner.tab_text_color = colors.solid;
        self
    }

    /// Immutable access to the tab container style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the tab container style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }

    fn emit_if_changed(&mut self, before: TabId) {
        let after = self.inner.active_tab();
        if before != after
            && let Some(cb) = self.on_change.as_mut()
        {
            cb(after);
        }
    }
}

impl Widget for Tabs {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        self.inner.widget_font_mut()
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.inner.set_bounds(bounds);
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
    use alloc::rc::Rc;
    use core::cell::Cell;

    #[test]
    fn tabs_change_callback_fires_on_navigation() {
        let active = Rc::new(Cell::new(Tabs::TAB_NONE));
        let flag = active.clone();
        let mut tabs = Tabs::new(rect(0, 0, 200, 100), TabBarPos::Top)
            .tab("A")
            .tab("B")
            .on_change(move |id| flag.set(id));

        tabs.navigate_next_tab();

        assert_eq!(active.get(), TabId(1));
        assert_eq!(tabs.active_tab(), TabId(1));
    }

    #[test]
    fn tabs_themed_sets_active_and_inactive_colors() {
        let theme = Theme::material_light();
        let tabs = Tabs::new(rect(0, 0, 200, 100), TabBarPos::Top).themed(
            &theme,
            ColorScheme::Primary,
            Variant::Solid,
            ComponentSize::Md,
        );

        assert_eq!(tabs.style().bg_color, theme.tokens.colors.primary);
        assert_eq!(
            tabs.inner.tab_color,
            theme.scheme(ColorScheme::Primary).subtle
        );
    }

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}
