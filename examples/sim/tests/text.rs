//! Tests for simulator text rendering baseline alignment and boundary clipping.

use rlvgl::core::{renderer::Renderer, widget::Color};
use rlvgl_sim::PixelsRenderer;

#[test]
fn descenders_extend_below_baseline() {
    let width = 32;
    let height = 32;
    let baseline = 16;
    for ch in ['g', 'q'] {
        let mut buf = vec![0u8; width * height * 4];
        {
            let mut renderer = PixelsRenderer::new(&mut buf, width, height);
            renderer.draw_text((0, baseline), &ch.to_string(), Color(0xff, 0xff, 0xff));
        }
        let mut found = false;
        for y in (baseline as usize + 1)..height {
            for x in 0..width {
                if buf[(y * width + x) * 4 + 3] != 0 {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        assert!(found, "character {ch} should render below the baseline");
    }
}

#[test]
fn draw_text_clips_to_frame_bounds() {
    let width = 20;
    let height = 20;
    let mut frame = vec![0u8; width * height * 4 + 16];
    let (buf, sentinel) = frame.split_at_mut(width * height * 4);
    let sentinel_before = sentinel.to_vec();
    {
        let mut renderer = PixelsRenderer::new(buf, width, height);
        // Partially outside to the left and bottom
        renderer.draw_text((-5, height as i32 - 1), "gq", Color(0xff, 0xff, 0xff));
        // Completely outside the frame
        renderer.draw_text(
            (width as i32 + 100, height as i32 + 100),
            "gq",
            Color(0xff, 0xff, 0xff),
        );
    }
    assert_eq!(sentinel, &sentinel_before[..], "sentinel bytes modified");
}
