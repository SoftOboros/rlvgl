//! Tests verifying text rendering alignment and clipping.
use rlvgl::fontdue::{line_metrics, rasterize_glyph};

const FONT_DATA: &[u8] = include_bytes!("../../../lvgl/scripts/built_in_font/DejaVuSans.ttf");

/// Convert a grayscale framebuffer into an ASCII art representation.
fn dump_ascii_frame(buffer: &[u8], width: usize, height: usize) -> String {
    let mut out = String::with_capacity((width + 1) * height);
    for y in 0..height {
        for x in 0..width {
            let val = buffer[y * width + x];
            let ch = match val {
                0 => ' ',
                1..=63 => '.',
                64..=127 => ':',
                128..=191 => '*',
                192..=223 => '#',
                _ => '@',
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

/// Render `text` into the provided grayscale framebuffer using a bottom-aligned baseline.
fn render_text_to_framebuffer(
    text: &str,
    fb: &mut [u8],
    width: usize,
    height: usize,
    size: f32,
    bottom_y: i32,
) {
    let baseline_y = bottom_y;
    let mut x_cursor = 0f32;
    for ch in text.chars() {
        if let Ok((bitmap, metrics)) = rasterize_glyph(FONT_DATA, ch, size) {
            let draw_y = baseline_y + metrics.ymin;
            for y in 0..metrics.height {
                let py = draw_y + y as i32;
                if py < 0 || py as usize >= height {
                    continue;
                }
                for x in 0..metrics.width {
                    let px = x_cursor as i32 + metrics.xmin + x as i32;
                    if px < 0 || px as usize >= width {
                        continue;
                    }
                    let alpha = bitmap[y * metrics.width + x];
                    fb[py as usize * width + px as usize] = alpha;
                }
            }
            x_cursor += metrics.advance_width;
        }
    }
}

/// Render `text` with its top row positioned at `top_y`.
fn render_text_top_aligned_to_framebuffer(
    text: &str,
    fb: &mut [u8],
    width: usize,
    height: usize,
    size: f32,
    top_y: i32,
) {
    let mut x_cursor = 0f32;
    for ch in text.chars() {
        if let Ok((bitmap, metrics)) = rasterize_glyph(FONT_DATA, ch, size) {
            let draw_y = top_y;
            for y in 0..metrics.height {
                let py = draw_y + y as i32;
                if py < 0 || py as usize >= height {
                    continue;
                }
                for x in 0..metrics.width {
                    let px = x_cursor as i32 + metrics.xmin + x as i32;
                    if px < 0 || px as usize >= width {
                        continue;
                    }
                    let alpha = bitmap[y * metrics.width + x];
                    fb[py as usize * width + px as usize] = alpha;
                }
            }
            x_cursor += metrics.advance_width;
        }
    }
}

#[test]
fn test_descenders_align_below_baseline() {
    const W: usize = 320;
    const H: usize = 240;
    let mut fb = vec![0u8; W * H];
    let baseline = 200i32;
    render_text_to_framebuffer("gpq", &mut fb, W, H, 16.0, baseline);
    let ascii = dump_ascii_frame(&fb, W, H);
    assert!(
        ascii
            .lines()
            .nth(baseline as usize)
            .unwrap()
            .chars()
            .any(|c| c != ' ')
    );
    assert!(
        ascii
            .lines()
            .nth(baseline as usize + 1)
            .unwrap()
            .chars()
            .any(|c| c != ' ')
    );
}

#[test]
fn test_clipped_bottom_text_does_not_panic() {
    const W: usize = 320;
    const H: usize = 240;
    let mut fb = vec![0u8; W * H];
    let bottom_y = H as i32 - 1;
    render_text_to_framebuffer("pqgy", &mut fb, W, H, 16.0, bottom_y);
    let ascii = dump_ascii_frame(&fb, W, H);
    assert!(ascii.lines().last().unwrap().chars().any(|c| c != ' '));
}

#[test]
fn test_top_aligned_text_differs_from_baseline() {
    const W: usize = 60;
    const H: usize = 30;
    let vm = line_metrics(FONT_DATA, 16.0).unwrap();
    let top_y = 5;
    let baseline = top_y + vm.ascent.round() as i32;
    let mut baseline_buf = vec![0u8; W * H];
    render_text_to_framebuffer("Hi", &mut baseline_buf, W, H, 16.0, baseline);
    let mut top_buf = vec![0u8; W * H];
    render_text_top_aligned_to_framebuffer("Hi", &mut top_buf, W, H, 16.0, top_y);
    assert_ne!(baseline_buf, top_buf);
}
