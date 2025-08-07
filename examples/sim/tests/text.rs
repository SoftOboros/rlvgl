//! Tests verifying text rendering alignment and clipping.
use rlvgl::core::{renderer::Renderer, widget::Color};
use rlvgl::fontdue::line_metrics;
use rlvgl_sim::PixelsRenderer;

const FONT_DATA: &[u8] = include_bytes!("../../../lvgl/scripts/built_in_font/DejaVuSans.ttf");

#[test]
fn test_descenders_render_below_baseline() {
    const W: usize = 50;
    const H: usize = 30;
    let mut frame = vec![0u8; W * H * 4];
    let bottom_y = 20;
    {
        let mut renderer = PixelsRenderer::new(&mut frame, W, H);
        renderer.draw_text((5, bottom_y), "gpq", Color(255, 255, 255));
    }
    let vm = line_metrics(FONT_DATA, 16.0).unwrap();
    let baseline_y = bottom_y - vm.descent.round() as i32;
    let mut descender_found = false;
    for y in (baseline_y + 1).max(0) as usize..H {
        for x in 0..W {
            let idx = (y * W + x) * 4 + 3;
            if frame[idx] != 0 {
                descender_found = true;
                break;
            }
        }
        if descender_found {
            break;
        }
    }
    assert!(descender_found);
}

#[test]
fn test_clipped_bottom_text_does_not_panic() {
    const W: usize = 40;
    const H: usize = 12;
    let mut frame = vec![0u8; W * H * 4];
    let bottom_y = H as i32 - 1;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut renderer = PixelsRenderer::new(&mut frame, W, H);
        renderer.draw_text((0, bottom_y), "hello", Color(255, 255, 255));
    }));
    assert!(result.is_ok());
}

#[test]
fn test_top_aligned_text_differs_from_baseline() {
    const W: usize = 60;
    const H: usize = 30;
    let vm = line_metrics(FONT_DATA, 16.0).unwrap();
    let top_y = 5;
    let bottom_y_top = top_y + vm.ascent.round() as i32 + vm.descent.round() as i32;

    let mut baseline_buf = vec![0u8; W * H * 4];
    {
        let mut r = PixelsRenderer::new(&mut baseline_buf, W, H);
        r.draw_text((5, bottom_y_top), "Hi", Color(255, 255, 255));
    }
    let mut top_buf = vec![0u8; W * H * 4];
    {
        let mut r = PixelsRenderer::new(&mut top_buf, W, H);
        r.draw_text((5, top_y), "Hi", Color(255, 255, 255));
    }
    assert_ne!(baseline_buf, top_buf);
}
