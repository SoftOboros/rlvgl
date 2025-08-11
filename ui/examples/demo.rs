// SPDX-License-Identifier: MIT
//! Minimal rlvgl-ui style demo.
//!
//! Builds a style using tokens from a Material-like theme.

#[cfg(feature = "view")]
use rlvgl_ui::view;
use rlvgl_ui::{
    Alert, Badge, Button, Checkbox, Drawer, Heading, Icon, IconButton, Input, Modal, OnClick,
    Radio, StyleBuilder, Switch, Tag, Text, Textarea, Theme, Toast, VStack,
};

fn main() {
    let theme = Theme::material_light();
    let style = StyleBuilder::new()
        .bg(theme.tokens.colors.primary)
        .radius(theme.tokens.radii.md)
        .build();

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

    let _ = (style, layout);
}
