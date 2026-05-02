//! Correctness tests for the `Renderer::blend_row` override on
//! `PixelsRenderer`. The override is the AA inner-loop hot path — every
//! `fill_obb_aa` call funnels coverage spans through it — so a regression
//! here silently degrades clock-hand sub-pixel quality.
//!
//! Two checks:
//! 1. Direct byte assertions on a known coverage pattern over a known
//!    background, to lock down the source-over math.
//! 2. Cross-check against the trait default (per-pixel `blend_rect`) — the
//!    override must produce bit-identical output, otherwise the same OBB
//!    would render differently on backends with vs. without the override.

#![cfg(feature = "simulator")]

use rlvgl_core::raster::{Obb, PointF};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};
use rlvgl_platform::pixels_renderer::PixelsRenderer;

const W: usize = 32;
const H: usize = 16;

fn black_buf() -> Vec<u8> {
    let mut buf = vec![0u8; W * H * 4];
    for i in (0..buf.len()).step_by(4) {
        buf[i] = 0; // R
        buf[i + 1] = 0; // G
        buf[i + 2] = 0; // B
        buf[i + 3] = 0xff; // A
    }
    buf
}

#[test]
fn blend_row_matches_source_over_math() {
    // Full-alpha red onto black background. Per-pixel result for each
    // coverage value c: R = (255 * c) / 255 = c; G = 0; B = 0; A = 255.
    let mut buf = black_buf();
    {
        let mut r = PixelsRenderer::new(&mut buf, W, H);
        r.blend_row(
            4,
            5,
            Color(255, 0, 0, 255),
            &[0, 32, 64, 96, 128, 160, 192, 224, 255],
        );
    }
    let row_base = 5 * W * 4;
    for (i, &cov) in [0, 32, 64, 96, 128, 160, 192, 224, 255].iter().enumerate() {
        let px = row_base + (4 + i) * 4;
        if cov == 0 {
            // Skip pixel: bg untouched (still black).
            assert_eq!(buf[px], 0, "cov 0 should leave R untouched");
            continue;
        }
        // bg=0, alpha=cov, R = (255*cov + 0*(255-cov))/255 = cov
        let expected_r = cov;
        let actual_r = buf[px];
        assert!(
            (actual_r as i32 - expected_r as i32).abs() <= 1,
            "pixel {i} cov={cov}: expected R~{expected_r}, got {actual_r}"
        );
        assert_eq!(buf[px + 1], 0, "G should remain 0");
        assert_eq!(buf[px + 2], 0, "B should remain 0");
        assert_eq!(buf[px + 3], 0xff, "A should be 0xff");
    }
}

#[test]
fn blend_row_overrride_matches_trait_default() {
    // Render an OBB via two paths — `fill_obb_aa` against
    // `PixelsRenderer` (uses overridden `blend_row`), and the same OBB via
    // a wrapper renderer that forces the trait default (per-pixel
    // `blend_rect` fallback). Outputs must match bit-for-bit.

    let obb = Obb::axis_aligned(PointF::new(16.0, 8.0), 14.0, 4.0);
    let color = Color(200, 100, 50, 255);

    let mut buf_override = black_buf();
    {
        let mut r = PixelsRenderer::new(&mut buf_override, W, H);
        r.fill_obb_aa(obb, color);
    }

    let mut buf_default = black_buf();
    {
        let mut inner = PixelsRenderer::new(&mut buf_default, W, H);
        let mut r = ForceDefaultBlendRow { inner: &mut inner };
        r.fill_obb_aa(obb, color);
    }

    let mut diffs = 0u32;
    for i in 0..buf_override.len() {
        if buf_override[i] != buf_default[i] {
            diffs += 1;
        }
    }
    assert_eq!(
        diffs, 0,
        "override path produced {diffs} byte differences from trait default"
    );
}

/// Wrapper that delegates everything to `inner` *except* `blend_row`,
/// which it leaves at the trait's default (per-pixel `blend_rect`). Used
/// to cross-check that the platform's `blend_row` override is
/// observationally identical to the unoptimized path.
struct ForceDefaultBlendRow<'a, R: Renderer> {
    inner: &'a mut R,
}

impl<R: Renderer> Renderer for ForceDefaultBlendRow<'_, R> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.inner.fill_rect(rect, color);
    }
    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        self.inner.draw_text(position, text, color);
    }
    fn blend_rect(&mut self, rect: Rect, color: Color) {
        self.inner.blend_rect(rect, color);
    }
    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        self.inner.draw_pixels(position, pixels, width, height);
    }
    // Note: NO `blend_row` override here — inherits the trait default,
    // which falls back to per-pixel `blend_rect` calls on `self`.
}
