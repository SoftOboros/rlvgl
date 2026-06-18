// SPDX-License-Identifier: MIT
//! Minimal rlvgl-ui style demo.
//!
//! Builds a style using tokens from a Material-like theme.
//!
//! This demo intentionally exercises the legacy `rlvgl_ui` style surface
//! (`StyleBuilder`/`Style`), deprecated in LPAR-07 in favour of the
//! `core::style_cascade` cascade but kept compiling for compatibility.
#![allow(deprecated)]

#[cfg(feature = "view")]
use rlvgl_ui::view;
use rlvgl_ui::{
    Alert, Badge, Bar, BarMode, Button, ButtonMatrix, Calendar, Checkbox, ColorScheme,
    ComponentSize, Divider, DividerOrientation, Drawer, Heading, Icon, IconButton, Input, Led,
    List, Modal, OnClick, Progress, Radio, RectProps, Select, Spacer, Spinner, StyleBuilder,
    StyleProps, Switch, TabBarPos, Table, Tabs, Tag, Text, Textarea, Theme, ThemedPartsProps,
    Toast, VStack, Variant, rect,
};

fn main() {
    let theme = Theme::material_light();
    let style = StyleBuilder::new()
        .bg(theme.tokens.colors.primary)
        .radius(theme.tokens.radii.md)
        .build();

    let new_components = (
        Progress::new(theme.control_rect(0, 0, 90, ComponentSize::Sm), 0, 100)
            .with_value(64)
            .themed(
                &theme,
                ColorScheme::Primary,
                Variant::Subtle,
                ComponentSize::Md,
            )
            .opacity(220),
        Select::new(theme.control_rect(0, 10, 90, ComponentSize::Sm).height(64))
            .with_options(&["One", "Two", "Three"])
            .with_selected_index(1)
            .themed(
                &theme,
                ColorScheme::Info,
                Variant::Outline,
                ComponentSize::Sm,
            )
            .on_change(|idx, text| println!("select: {idx} {text}")),
        Tabs::new(rect(0, 80, 120, 80), TabBarPos::Top)
            .tab("Main")
            .tab("Settings")
            .themed(
                &theme,
                ColorScheme::Primary,
                Variant::Solid,
                ComponentSize::Md,
            ),
        ButtonMatrix::new(
            theme
                .control_rect(0, 170, 120, ComponentSize::Md)
                .height(48),
        )
        .with_map(&["A", "B", "\n", "C", "D"])
        .themed(
            &theme,
            ColorScheme::Neutral,
            Variant::Outline,
            ComponentSize::Md,
        )
        .on_activate(|id, text| println!("button matrix: {:?} {text}", id)),
        Bar::new(theme.control_rect(0, 224, 90, ComponentSize::Xs), 0, 100)
            .with_value(32)
            .with_mode(BarMode::Normal)
            .themed(
                &theme,
                ColorScheme::Success,
                Variant::Subtle,
                ComponentSize::Sm,
            ),
        Led::new(
            theme
                .origin_control_rect(12, ComponentSize::Xs)
                .at(0, 240)
                .size(12, 12),
        )
        .themed(
            &theme,
            ColorScheme::Success,
            Variant::Solid,
            ComponentSize::Xs,
        )
        .brightness(180),
        Spinner::new(theme.origin_control_rect(24, ComponentSize::Md).at(20, 236))
            .animation(30, 100)
            .themed(
                &theme,
                ColorScheme::Primary,
                Variant::Ghost,
                ComponentSize::Md,
            ),
        List::new(
            theme
                .control_rect(0, 270, 100, ComponentSize::Sm)
                .height(48),
        )
        .with_items(&["Alpha", "Beta", "Gamma"])
        .themed(
            &theme,
            ColorScheme::Neutral,
            Variant::Subtle,
            ComponentSize::Sm,
        ),
        Divider::new(rect(0, 328, 100, 8), DividerOrientation::Horizontal).themed(
            &theme,
            ColorScheme::Neutral,
            Variant::Outline,
            ComponentSize::Sm,
        ),
        Spacer::height(theme.component_size(ComponentSize::Md).gap),
        Table::new(rect(0, 342, 120, 48)).themed_parts(
            &theme,
            ColorScheme::Info,
            Variant::Subtle,
            ComponentSize::Sm,
        ),
        Calendar::new(rect(0, 396, 140, 96)).themed_parts(
            &theme,
            ColorScheme::Primary,
            Variant::Outline,
            ComponentSize::Sm,
        ),
    );

    let layout = {
        #[cfg(feature = "view")]
        {
            view! { VStack::new(90)
            .child(20, |rect| Heading::new("Demo", rect))
            .child(20, |rect| Text::new("Hello", rect))
            .child(30, |rect| {
                Button::new("Tap", rect)
                    .icon("save")
                    .on_click(|_| println!("clicked"))
            })
            .child(30, |rect| {
                IconButton::new("edit", rect)
                    .on_click(|_| println!("edit"))
            })
            .child(20, |rect| {
                Checkbox::new("Accept", rect).on_change(|v| println!("checkbox: {v}"))
            })
            .child(20, |rect| {
                Switch::new(rect).on_change(|v| println!("switch: {v}"))
            })
            .child(20, |rect| {
                Radio::new("Option", rect).on_change(|v| println!("radio: {v}"))
            })
            .child(20, |rect| { Badge::new("NEW", rect) })
            .child(20, |rect| {
                Tag::new("rust", rect).on_remove(|| println!("tag removed"))
            })
            .child(30, |rect| { Alert::new("Saved", rect) })
            .child(20, |rect| {
                Input::new("Name", rect).on_change(|v| println!("input: {v}"))
            })
            .child(40, |rect| {
                Textarea::new("Multiline", rect).on_change(|v| println!("textarea: {v}"))
            })
            .child(30, |rect| Modal::new("Modal", rect))
            .child(30, |rect| Drawer::new("Menu", rect))
            .child(30, |rect| Toast::new("Saved", rect)) }
        }
        #[cfg(not(feature = "view"))]
        {
            VStack::new(90)
                .child(20, |rect| Heading::new("Demo", rect))
                .child(20, |rect| Text::new("Hello", rect))
                .child(30, |rect| {
                    Button::new("Tap", rect)
                        .icon("save")
                        .on_click(|_| println!("clicked"))
                })
                .child(30, |rect| {
                    IconButton::new("edit", rect).on_click(|_| println!("edit"))
                })
                .child(20, |rect| {
                    Checkbox::new("Accept", rect).on_change(|v| println!("checkbox: {v}"))
                })
                .child(20, |rect| {
                    Switch::new(rect).on_change(|v| println!("switch: {v}"))
                })
                .child(20, |rect| {
                    Radio::new("Option", rect).on_change(|v| println!("radio: {v}"))
                })
                .child(20, |rect| Badge::new("NEW", rect))
                .child(20, |rect| {
                    Tag::new("rust", rect).on_remove(|| println!("tag removed"))
                })
                .child(30, |rect| Alert::new("Saved", rect))
                .child(20, |rect| {
                    Input::new("Name", rect).on_change(|v| println!("input: {v}"))
                })
                .child(40, |rect| {
                    Textarea::new("Multiline", rect).on_change(|v| println!("textarea: {v}"))
                })
                .child(30, |rect| Modal::new("Modal", rect))
                .child(30, |rect| Drawer::new("Menu", rect))
                .child(30, |rect| Toast::new("Saved", rect))
        }
    };

    let _ = (style, new_components, layout);
}
