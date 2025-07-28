#[cfg(feature = "simulator")]
use platform::display::DisplayDriver;
#[cfg(feature = "simulator")]
use platform::MinifbDisplay;
#[cfg(feature = "simulator")]
use rlvgl_core::widget::{Color, Rect};

#[cfg(feature = "simulator")]
#[test]
fn minifb_window_draws() {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("skipping minifb_window_draws: no display");
        return;
    }
    let mut disp = MinifbDisplay::new(4, 4);
    let area = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    let colors = [Color(5, 10, 15); 16];
    disp.flush(area, &colors);
    // success is not crashing when calling flush
}

#[cfg(not(feature = "simulator"))]
#[test]
fn minifb_window_draws() {
    // Simulator feature not enabled; nothing to test
    assert!(true);
}
