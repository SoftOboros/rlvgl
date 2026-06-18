// SPDX-License-Identifier: MIT
//! Prop extensions for directly re-exported LVGL-parity widgets.
//!
//! Some mature widgets are re-exported from `rlvgl-widgets` rather than wrapped
//! in new `rlvgl-ui` types. This module adds local fluent props for those
//! widgets without introducing conflicting wrapper names.

use rlvgl_widgets::{
    calendar::Calendar, message_box::MessageBox, spinbox::Spinbox, table::Table, window::Window,
};

use crate::{
    style::Color,
    theme::{ColorScheme, ComponentSize, ComponentStyle, SchemeColors, Theme, Variant},
};

/// Fluent themed part colors for direct LVGL-parity widget re-exports.
///
/// Direct widgets already support generic [`ThemeProps`](crate::ThemeProps)
/// because their main `style` fields are public. Use `themed_parts` when the
/// widget also has public colors for selected rows, headers, cursors, or
/// header buttons.
pub trait ThemedPartsProps: Sized {
    /// Apply the theme to the main style and direct widget-specific part colors.
    fn themed_parts(
        self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self;
}

impl ThemedPartsProps for Calendar {
    fn themed_parts(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        let colors = theme.scheme(scheme);
        self.style = resolved.style;
        self.day_cell_color = panel_color(resolved, colors);
        self.selected_color = resolved.accent_color;
        self.today_color = theme.scheme(ColorScheme::Warning).solid;
        self.text_color = resolved.text_color;
        self.header_text_color = colors.solid;
        self.overflow_text_color = colors.border;
        self
    }
}

impl ThemedPartsProps for MessageBox {
    fn themed_parts(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        self.style = resolved.style;
        self.text_color = resolved.text_color;
        self.button_text_color = resolved.accent_color;
        self
    }
}

impl ThemedPartsProps for Spinbox {
    fn themed_parts(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        self.style = resolved.style;
        self.text_color = resolved.text_color;
        self.cursor_color = resolved.accent_color;
        self
    }
}

impl ThemedPartsProps for Table {
    fn themed_parts(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        let colors = theme.scheme(scheme);
        self.style = resolved.style;
        self.cell_bg_color = panel_color(resolved, colors);
        self.selected_color = resolved.accent_color;
        self.text_color = resolved.text_color;
        self.grid_color = colors.border;
        self
    }
}

impl ThemedPartsProps for Window {
    fn themed_parts(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        let colors = theme.scheme(scheme);
        self.style = resolved.style;
        self.header_color = resolved.accent_color;
        self.title_color = colors.contrast;
        self.button_color = colors.muted;
        self.button_text_color = colors.solid;
        self
    }
}

fn panel_color(resolved: ComponentStyle, colors: SchemeColors) -> Color {
    if resolved.style.bg_color.3 == 0 {
        colors.subtle
    } else {
        resolved.style.bg_color
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Rect;
    use rlvgl_widgets::window::DEFAULT_HEADER_HEIGHT;

    #[test]
    fn table_themed_parts_apply_cell_and_selection_colors() {
        let theme = Theme::material_light();
        let table = Table::new(rect(0, 0, 100, 60)).themed_parts(
            &theme,
            ColorScheme::Info,
            Variant::Outline,
            ComponentSize::Lg,
        );

        assert_eq!(table.style.border_width, 2);
        assert_eq!(table.cell_bg_color, theme.scheme(ColorScheme::Info).subtle);
        assert_eq!(table.selected_color, theme.scheme(ColorScheme::Info).solid);
        assert_eq!(table.grid_color, theme.scheme(ColorScheme::Info).border);
    }

    #[test]
    fn calendar_themed_parts_apply_calendar_specific_colors() {
        let theme = Theme::material_light();
        let calendar = Calendar::new(rect(0, 0, 140, 120)).themed_parts(
            &theme,
            ColorScheme::Success,
            Variant::Subtle,
            ComponentSize::Md,
        );

        assert_eq!(
            calendar.selected_color,
            theme.scheme(ColorScheme::Success).solid
        );
        assert_eq!(
            calendar.today_color,
            theme.scheme(ColorScheme::Warning).solid
        );
        assert_eq!(
            calendar.header_text_color,
            theme.scheme(ColorScheme::Success).solid
        );
    }

    #[test]
    fn text_entry_direct_widgets_receive_accent_colors() {
        let theme = Theme::material_light();
        let spinbox = Spinbox::new(rect(0, 0, 80, 24)).themed_parts(
            &theme,
            ColorScheme::Danger,
            Variant::Ghost,
            ComponentSize::Sm,
        );
        let message_box = MessageBox::new(rect(0, 0, 120, 80), "Title", "Body", &["OK"])
            .themed_parts(
                &theme,
                ColorScheme::Danger,
                Variant::Ghost,
                ComponentSize::Sm,
            );

        assert_eq!(
            spinbox.cursor_color,
            theme.scheme(ColorScheme::Danger).solid
        );
        assert_eq!(
            message_box.button_text_color,
            theme.scheme(ColorScheme::Danger).solid
        );
    }

    #[test]
    fn window_themed_parts_apply_header_palette() {
        let theme = Theme::material_light();
        let window = Window::new(rect(0, 0, 120, 80), DEFAULT_HEADER_HEIGHT).themed_parts(
            &theme,
            ColorScheme::Primary,
            Variant::Solid,
            ComponentSize::Md,
        );

        assert_eq!(
            window.header_color,
            theme.scheme(ColorScheme::Primary).solid
        );
        assert_eq!(
            window.title_color,
            theme.scheme(ColorScheme::Primary).contrast
        );
        assert_eq!(
            window.button_color,
            theme.scheme(ColorScheme::Primary).muted
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
