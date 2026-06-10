//! CRATES-CI-01 lib-smoke Consumer Project (CRATES-CI-00 §8, Gate P/R).
//!
//! Forces feature resolution + linking of `rlvgl-core` (`png`, `fontdue`)
//! and `rlvgl-widgets` from PACKAGED crates — the P-RESOLVE failure class
//! repaired by commit 9bee2f9. Not a rendering test.

use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::{button::Button, label::Label};

fn main() {
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 32,
    };
    let mut label = Label::new("crates-ci", bounds);
    label.style = Style::default();
    label.text_color = Color(255, 255, 255, 255);
    assert_eq!(label.text(), "crates-ci");

    let button = Button::new(
        "ok",
        Rect {
            x: 0,
            y: 40,
            width: 80,
            height: 24,
        },
    );
    assert_eq!(button.bounds().width, 80);

    // Feature-gated plugin paths: both must resolve and link from the
    // packaged rlvgl-core (P-RESOLVE). Garbage input is expected to Err.
    assert!(rlvgl_core::png::decode(&[0u8; 4]).is_err());
    assert!(rlvgl_core::fontdue::line_metrics(&[0u8; 4], 12.0).is_err());

    println!("OK");
}
