// SPDX-License-Identifier: MIT
//! High-level style and theme utilities for rlvgl UI components.

#![no_std]

extern crate alloc;

pub mod alert;
pub mod badge;
pub mod button;
pub mod checkbox;
pub mod drawer;
pub mod event;
pub mod icon;
pub mod input;
pub mod layout;
pub mod modal;
pub mod radio;
pub mod style;
pub mod switch;
pub mod tag;
pub mod text;
pub mod theme;
pub mod toast;
#[cfg(feature = "view")]
pub mod view;

pub use alert::Alert;
pub use badge::Badge;
pub use button::IconButton;
pub use checkbox::Checkbox;
pub use drawer::Drawer;
pub use event::{OnClick, Slider};
pub use icon::{Icon, lookup};
pub use input::{Input, Textarea};
pub use layout::{BoxLayout, Grid, HStack, VStack};
pub use modal::Modal;
pub use radio::Radio;
pub use rlvgl_widgets::button::Button;
pub use style::{Color, Part, State, Style, StyleBuilder};
pub use switch::Switch;
pub use tag::Tag;
pub use text::{Heading, Text};
pub use theme::{Theme, Tokens};
pub use toast::Toast;
