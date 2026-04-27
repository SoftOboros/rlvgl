// SPDX-License-Identifier: MIT
//
// rust_inline_v1 layout fragment for the Beetle ESP32-C3 esp_hal intent.
// Cited by ../app.yaml as `screens[].layout`. Per
// docs/app-schema/02-generator-pipeline.md §7.7, the orchestrator copies
// this file verbatim into src/screens/main-screen.rs.
//
// Status: candidate fragment. The existing src/esp_hal_main.rs holds the
// equivalent loop body inline (lines 56-73); decomposing the binary so this
// fragment is the canonical source-of-truth is a follow-up under APP-03a.
// Until then, this file documents the intended screen body and IS NOT
// referenced by any current build.

use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};
use rlvgl_platform::display::DisplayDriver;

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 128,
    height: 64,
};

const WHITE: Color = Color(255, 255, 255, 255);
const BLACK: Color = Color(0, 0, 0, 255);

/// Render one frame of the Beetle ESP32-C3 demo.
///
/// Called once per tick from the prong's main glue (see
/// docs/app-schema/02-generator-pipeline.md §8.2 bare_metal template).
/// `frame` is a wrapping counter the caller increments per tick.
pub fn render<D>(display: &mut D, frame: u32)
where
    D: DisplayDriver + Renderer,
{
    display.fill_rect(SCREEN, BLACK);
    display.draw_text((2, 12), "rlvgl", WHITE);
    display.draw_text((2, 24), "Beetle ESP32-C3", WHITE);
    display.draw_text((2, 36), "SSD1306 128x64", WHITE);

    let bar = Rect {
        x: 2,
        y: 52,
        width: ((frame % 64) as i32).max(1),
        height: 4,
    };
    display.fill_rect(bar, WHITE);
}
