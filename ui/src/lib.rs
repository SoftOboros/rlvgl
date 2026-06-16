// SPDX-License-Identifier: MIT
//! High-level style and theme utilities bridging
//! [`rlvgl-core`](rlvgl_core) and [`rlvgl-widgets`](rlvgl_widgets)
//! components.

#![no_std]

extern crate alloc;

pub mod alert;
pub mod badge;
pub mod bar;
pub mod button;
pub mod button_matrix;
pub mod checkbox;
pub mod direct_props;
pub mod divider;
pub mod draw_helpers;
pub mod drawer;
pub mod event;
pub mod event_window;
pub mod file_browser;
pub mod icon;
pub mod input;
pub mod layout;
pub mod led;
pub mod list;
pub mod modal;
pub mod progress;
pub mod props;
pub mod radio;
pub mod select;
pub mod spacer;
pub mod spinner;
pub mod style;
pub mod switch;
pub mod tabs;
pub mod tag;
pub mod text;
pub mod theme;
pub mod toast;
#[cfg(feature = "view")]
pub mod view;

pub use alert::Alert;
pub use badge::Badge;
pub use bar::{Bar, BarMode, BarOrientation};
pub use button::IconButton;
pub use button_matrix::{BUTTON_NONE, ButtonId, ButtonMatrix, ButtonMatrixControl};
pub use checkbox::Checkbox;
pub use direct_props::ThemedPartsProps;
pub use divider::{Divider, DividerOrientation};
pub use drawer::Drawer;
pub use event::{OnClick, Slider};
pub use event_window::{EventWindow, EventWindowBuilder};
pub use icon::{Icon, lookup};
pub use input::{Input, Textarea};
pub use layout::{BoxLayout, Grid, GridCalc, HStack, RectProps, VStack, origin_rect, rect};
pub use led::Led;
pub use list::List;
pub use modal::Modal;
pub use progress::Progress;
pub use props::{BoundsProps, StyleProps, ThemeProps};
pub use radio::Radio;
pub use rlvgl_widgets::button::Button;
pub use rlvgl_widgets::calendar::{Calendar, CalendarDate};
pub use rlvgl_widgets::message_box::MessageBox;
pub use rlvgl_widgets::spinbox::{Spinbox, SpinboxDigitStepDirection};
pub use rlvgl_widgets::table::{CellAlign, CellCtrl, Table};
pub use rlvgl_widgets::window::{DEFAULT_HEADER_HEIGHT, Window, WindowButtonId};
pub use select::{Select, SelectDirection};
pub use spacer::Spacer;
pub use spinner::Spinner;
#[allow(deprecated)]
pub use style::{Color, Part, State, Style, StyleBuilder};
pub use switch::Switch;
pub use tabs::{TabBarPos, TabId, Tabs};
pub use tag::Tag;
pub use text::{Heading, Text};
pub use theme::{
    ColorScheme, ComponentSize, ComponentSizeTokens, ComponentStyle, SchemeColors, Theme, Tokens,
    Variant,
};
pub use toast::Toast;
