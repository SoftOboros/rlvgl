//! Rendering interface used by widgets.
//!
//! Implementors of this trait can target displays, off-screen buffers or
//! simulator windows.

use crate::cmd::CommandList;
use crate::raster::{self, CoverageSink, Obb};
use crate::widget::{Color, Rect};

/// Target-agnostic drawing interface.
///
/// Renderers are supplied to widgets during the draw phase. Implementations
/// may target a physical display, an off-screen buffer or a simulator window.
pub trait Renderer {
    /// Fill the given rectangle with a solid color.
    fn fill_rect(&mut self, rect: Rect, color: Color);

    /// Draw UTF‑8 text with its baseline anchored at the provided position using the color.
    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color);

    /// Blend a rectangle onto the target, honoring the alpha channel of `color`
    /// for source-over compositing.
    ///
    /// The default implementation ignores alpha and falls back to
    /// [`fill_rect`](Self::fill_rect). Backends with blending support should
    /// override this for correct anti-aliased rendering.
    fn blend_rect(&mut self, rect: Rect, color: Color) {
        self.fill_rect(rect, color);
    }

    /// Blit a buffer of pixels to the target at the given position.
    ///
    /// The default implementation falls back to per-pixel [`fill_rect`](Self::fill_rect)
    /// calls. Backends with bulk-copy support (e.g. DMA2D) should override this.
    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let idx = (y as u32 * width + x as u32) as usize;
                if let Some(&c) = pixels.get(idx) {
                    self.fill_rect(
                        Rect {
                            x: position.0 + x,
                            y: position.1 + y,
                            width: 1,
                            height: 1,
                        },
                        c,
                    );
                }
            }
        }
    }

    /// Blend a horizontal run of pixels with per-pixel anti-aliased coverage.
    ///
    /// `coverage[i]` modulates `color`'s alpha for the pixel at
    /// `(x + i, y)`. This is the AA inner-loop primitive: every higher-level
    /// AA method funnels coverage spans through here, so backends with
    /// hardware blend support (e.g. DMA2D's blend mode with per-pixel alpha
    /// modulation) should override this for performance.
    ///
    /// The default implementation walks the run via [`blend_rect`](Self::blend_rect)
    /// per pixel — correct, slow, sufficient for non-hot paths.
    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        for (i, &cov) in coverage.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let alpha = ((color.3 as u16 * cov as u16) / 255) as u8;
            self.blend_rect(
                Rect {
                    x: x + i as i32,
                    y,
                    width: 1,
                    height: 1,
                },
                Color(color.0, color.1, color.2, alpha),
            );
        }
    }

    /// Fill an oriented bounding box with anti-aliased coverage.
    ///
    /// `obb`'s center is in absolute framebuffer coordinates with sub-pixel
    /// precision; `theta` is supplied via pre-computed `(cos_t, sin_t)` on
    /// the [`Obb`] itself. `color`'s alpha is multiplied by per-pixel
    /// coverage before blending.
    ///
    /// The default implementation rasterizes via
    /// [`raster::rasterize_obb`] and emits coverage spans through
    /// [`blend_row`](Self::blend_row). Backends that have hardware OBB
    /// rasterization can override this directly; backends with hardware
    /// blend but software geometry should override `blend_row` instead and
    /// inherit this default.
    fn fill_obb_aa(&mut self, obb: Obb, color: Color) {
        let clip = obb.aabb();
        let mut sink = RowBlendSink { r: self, color };
        raster::rasterize_obb(&obb, clip, &mut sink);
    }

    /// Fill a disc (filled circle) with anti-aliased coverage at the
    /// boundary. Sub-pixel center; sqrt is restricted to the 1-pixel AA
    /// ring so the inner-area fast-path stays integer-arithmetic only.
    ///
    /// The default implementation routes through
    /// [`raster::rasterize_disc`] + [`blend_row`](Self::blend_row), so
    /// any backend that has overridden `blend_row` for hardware blend
    /// inherits the acceleration here automatically.
    fn fill_disc_aa(&mut self, center: crate::raster::PointF, radius: f32, color: Color) {
        let pad = radius + 1.0;
        let clip = Rect {
            x: (center.x - pad) as i32 - 1,
            y: (center.y - pad) as i32 - 1,
            width: (pad * 2.0) as i32 + 3,
            height: (pad * 2.0) as i32 + 3,
        };
        let mut sink = RowBlendSink { r: self, color };
        raster::rasterize_disc(center, radius, clip, &mut sink);
    }

    /// Stroke a line between `a` and `b` with given `width`, anti-aliased.
    /// Endpoints are square-cut; see [`raster::rasterize_line`].
    ///
    /// Default implementation routes through
    /// [`raster::rasterize_line`] + [`blend_row`](Self::blend_row), so
    /// `blend_row` overrides apply automatically.
    fn stroke_line_aa(
        &mut self,
        a: crate::raster::PointF,
        b: crate::raster::PointF,
        width: f32,
        color: Color,
    ) {
        // Conservative AABB: full canvas span — `rasterize_line` clips
        // internally to the OBB AABB anyway, so passing a permissive clip
        // here only costs a single rect-intersect inside the kernel.
        let clip = Rect {
            x: i32::MIN / 2,
            y: i32::MIN / 2,
            width: i32::MAX / 2,
            height: i32::MAX / 2,
        };
        let mut sink = RowBlendSink { r: self, color };
        raster::rasterize_line(a, b, width, clip, &mut sink);
    }

    /// Fill an annular arc / pie slice with anti-aliased coverage.
    /// See [`raster::rasterize_arc`] for the angle convention; in short,
    /// `(start_cos, start_sin)` and `(end_cos, end_sin)` are pre-computed
    /// boundary-ray unit vectors and `extent` is the *signed* angular
    /// magnitude. `r_inner = 0.0` produces a pie slice; `r_inner > 0.0`
    /// produces a ring segment.
    ///
    /// Default impl routes through [`raster::rasterize_arc`] +
    /// [`blend_row`](Self::blend_row).
    #[allow(clippy::too_many_arguments)]
    fn fill_arc_aa(
        &mut self,
        center: crate::raster::PointF,
        r_outer: f32,
        r_inner: f32,
        start_cos: f32,
        start_sin: f32,
        end_cos: f32,
        end_sin: f32,
        extent: f32,
        color: Color,
    ) {
        let pad = r_outer + 1.0;
        let clip = Rect {
            x: (center.x - pad) as i32 - 1,
            y: (center.y - pad) as i32 - 1,
            width: (pad * 2.0) as i32 + 3,
            height: (pad * 2.0) as i32 + 3,
        };
        let mut sink = RowBlendSink { r: self, color };
        raster::rasterize_arc(
            center, r_outer, r_inner, start_cos, start_sin, end_cos, end_sin, extent, clip,
            &mut sink,
        );
    }

    /// Execute a captured [`CommandList`] against this renderer.
    ///
    /// Default implementation walks the list and dispatches each
    /// command via [`crate::cmd::Cmd::dispatch_to`] — equivalent to
    /// having issued the corresponding trait calls directly. Backends
    /// override this to apply pre-pass optimizations: occlusion
    /// culling, opaque-cmd skip, hardware command-buffer chaining,
    /// tile binning. Overrides must preserve byte-identical output to
    /// the default path.
    ///
    /// This is the "graphics-language" entry point on the [`Renderer`]
    /// trait — code holding `&mut dyn Renderer` can submit captured
    /// command lists and pick up backend specializations
    /// transparently. See [`crate::cmd`] for the language model.
    fn submit(&mut self, list: &CommandList) {
        list.replay(self);
    }
}

struct RowBlendSink<'r, R: Renderer + ?Sized> {
    r: &'r mut R,
    color: Color,
}

impl<R: Renderer + ?Sized> CoverageSink for RowBlendSink<'_, R> {
    fn row(&mut self, x: i32, y: i32, coverage: &[u8]) {
        self.r.blend_row(x, y, self.color, coverage);
    }
}
