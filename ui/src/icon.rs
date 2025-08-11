// SPDX-License-Identifier: MIT
//! Icon font helpers and Button extension for rlvgl-ui.
//!
//! Maps human-readable icon names to LVGL built-in symbol codepoints and
//! provides a fluent `icon` method on buttons.

use alloc::string::{String, ToString};
use core::fmt::Write;
use rlvgl_widgets::button::Button;

/// Resolve a human-friendly icon name to an LVGL symbol string.
pub fn lookup(name: &str) -> Option<&'static str> {
    match name {
        "save" => Some("\u{f0c7}"),
        "edit" => Some("\u{f304}"),
        "close" => Some("\u{f00d}"),
        _ => None,
    }
}

/// Extension trait adding an `icon` method to buttons.
pub trait Icon {
    /// Prefix the button label with the specified icon, if known.
    fn icon(self, name: &str) -> Self;
}

impl Icon for Button {
    fn icon(mut self, name: &str) -> Self {
        if let Some(sym) = lookup(name) {
            let text = self.text().to_string();
            let mut buf = String::new();
            // Write formatted text into the buffer; writing to a String cannot fail.
            let _ = write!(&mut buf, "{sym} {text}");
            self.set_text(&buf);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Rect;

    #[test]
    fn icon_prefixes_label() {
        let btn = Button::new(
            "Save",
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        )
        .icon("save");
        assert!(btn.text().starts_with(lookup("save").unwrap()));
    }
}
