//! Tests for the pixels simulator window.
#[cfg(feature = "simulator")]
use rlvgl_platform::PixelsDisplay;

#[cfg(feature = "simulator")]
#[test]
fn pixels_window_draws() {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("skipping pixels_window_draws: no display");
        return;
    }
    let _disp = PixelsDisplay::new(4, 4);
    // success is not crashing when constructing the display
}

#[cfg(not(feature = "simulator"))]
#[test]
fn pixels_window_draws() {
    // Simulator feature not enabled; nothing to test
    assert!(true);
}
