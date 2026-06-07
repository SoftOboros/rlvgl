// SPDX-License-Identifier: MIT
//! Shared 747-style icon and layout assets for the disco demo controller.

/// Display width used by the original STM32H747I-DISCO demo.
pub const DISPLAY_WIDTH: i32 = 800;
/// Display height used by the original STM32H747I-DISCO demo.
pub const DISPLAY_HEIGHT: i32 = 480;
/// Width of the main detail panel.
pub const PANEL_WIDTH: i32 = 620;
/// Height of the main detail panel.
pub const PANEL_HEIGHT: i32 = 312;
/// X coordinate of the main detail panel.
pub const PANEL_X: i32 = 84;
/// Y coordinate of the main detail panel.
pub const PANEL_Y: i32 = 84;

/// Main-strip settings icon.
pub static ICON_SETTINGS: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/settings.rle");
/// Main-strip file icon.
pub static ICON_FILE: &[u8] = include_bytes!("../../../stm32h747i-disco/assets/icons/file.rle");
/// Main-strip info icon.
pub static ICON_INFO: &[u8] = include_bytes!("../../../stm32h747i-disco/assets/icons/info.rle");

/// Audio/action icon used in the settings and info wings.
pub static ICON_AUDIO_48: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/48/audio48.rle");
/// Camera placeholder icon used in the settings wing.
pub static ICON_CAMERA_48: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/48/camera48.rle");
/// Monitor icon used for display and live-stats actions.
pub static ICON_MONITOR_48: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/48/monitor48.rle");
/// Globe icon used for locale and platform status actions.
pub static ICON_GLOBE_48: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/48/globe48.rle");
/// Bug icon used for diagnostics and backlight actions.
pub static ICON_BUG_48: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/48/bug48.rle");
/// CPU icon used for diagnostic pages.
pub static ICON_CPU_48: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/48/cpu48.rle");
/// Play icon used for effect demos.
pub static ICON_PLAY_48: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/48/play48.rle");

/// Focus highlight border color (cyan accent).
pub const FOCUS_HIGHLIGHT_COLOR: rlvgl_core::widget::Color =
    rlvgl_core::widget::Color(0, 180, 255, 255);
/// Focus highlight border width in pixels.
pub const FOCUS_BORDER_WIDTH: u8 = 2;
