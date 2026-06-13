//! Rendering interface used by widgets.
//!
//! Implementors of this trait can target displays, off-screen buffers or
//! simulator windows.

use crate::cmd::CommandList;
use crate::draw::{GradientDesc, ShadowDesc};
use crate::font::ShapedText;
use crate::mask::AlphaMask;
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

    /// Draw a pre-shaped text run using glyph extent information.
    ///
    /// `origin` is an additional translation applied to every glyph placement
    /// in `shaped`. Pass `(0, 0)` when the shaped glyph positions are already
    /// in target coordinates. The default implementation renders glyph
    /// coverage when the shaped run carries a font reference; manually built
    /// shaped runs without font coverage fall back to a deterministic extent
    /// visualizer where each glyph extent is blended as a solid rectangle.
    /// Backends can override this for hardware text acceleration while
    /// preserving the same placement and clipping contract.
    fn draw_text_shaped(&mut self, shaped: &ShapedText<'_>, origin: (i32, i32), color: Color) {
        for glyph in &shaped.glyphs {
            let mut extent = glyph.extent();
            extent.x += origin.0;
            extent.y += origin.1;
            if extent.width <= 0 || extent.height <= 0 {
                continue;
            }
            if let Some(font) = shaped.font
                && draw_glyph_coverage(self, font, glyph.ch, extent, color)
            {
                continue;
            }
            self.blend_rect(extent, color);
        }
    }

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

    /// Fill `rect` with `color` modulated by an alpha mask.
    ///
    /// The default implementation evaluates the mask in fixed-size row
    /// chunks and emits coverage through [`blend_row`](Self::blend_row).
    /// Backends with hardware mask support may override this method, but the
    /// software path is the reference behavior.
    fn fill_masked(&mut self, rect: Rect, color: Color, mask: &dyn AlphaMask) {
        if rect.width <= 0 || rect.height <= 0 || color.3 == 0 {
            return;
        }
        let mut coverage = [0u8; 64];
        for y in rect.y..rect.y + rect.height {
            let mut x = rect.x;
            let end = rect.x + rect.width;
            while x < end {
                let run = (end - x).min(coverage.len() as i32) as usize;
                let row = &mut coverage[..run];
                mask.row(x, y, row);
                self.blend_row(x, y, color, row);
                x += run as i32;
            }
        }
    }

    /// Fill `rect` with a deterministic software gradient.
    ///
    /// The default path samples one pixel at a time using integer color
    /// interpolation. Opaque samples use [`fill_rect`](Self::fill_rect);
    /// translucent samples use [`blend_rect`](Self::blend_rect).
    fn fill_gradient(&mut self, rect: Rect, gradient: &GradientDesc<'_>) {
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                let Some(color) = gradient.color_at(rect, x, y) else {
                    continue;
                };
                if color.3 == 0 {
                    continue;
                }
                let pixel = Rect {
                    x,
                    y,
                    width: 1,
                    height: 1,
                };
                if color.3 == 255 {
                    self.fill_rect(pixel, color);
                } else {
                    self.blend_rect(pixel, color);
                }
            }
        }
    }

    /// Draw a deterministic software box shadow below `rect`.
    ///
    /// The v1 fallback uses a rounded-rect-compatible rectangular blur
    /// approximation. It is intentionally conservative and routes through
    /// [`fill_masked`](Self::fill_masked), so clipping adapters inherit the
    /// behavior through [`blend_row`](Self::blend_row).
    fn draw_shadow(&mut self, rect: Rect, radius: u8, shadow: &ShadowDesc) {
        if rect.width <= 0 || rect.height <= 0 || shadow.color.3 == 0 {
            return;
        }
        let spread = shadow.spread as i32;
        let blur = shadow.blur as i32;
        let base = Rect {
            x: rect.x + shadow.offset_x as i32 - spread,
            y: rect.y + shadow.offset_y as i32 - spread,
            width: rect.width + spread * 2,
            height: rect.height + spread * 2,
        };
        if base.width <= 0 || base.height <= 0 {
            return;
        }
        let draw_rect = Rect {
            x: base.x - blur,
            y: base.y - blur,
            width: base.width + blur * 2,
            height: base.height + blur * 2,
        };
        let mask = ShadowMask {
            base,
            blur,
            radius: radius as i32 + spread,
        };
        self.fill_masked(draw_rect, shadow.color, &mask);
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

fn draw_glyph_coverage<R: Renderer + ?Sized>(
    renderer: &mut R,
    font: &dyn crate::font::FontMetrics,
    ch: char,
    extent: Rect,
    color: Color,
) -> bool {
    let mut coverage = [0u8; 64];
    let height = extent.height.min(u16::MAX as i32) as u16;
    let width = extent.width.min(u16::MAX as i32) as u16;
    for row in 0..height {
        let mut x_offset = 0u16;
        while x_offset < width {
            let run = (width - x_offset).min(coverage.len() as u16) as usize;
            let row_coverage = &mut coverage[..run];
            if !font.glyph_coverage_row(ch, row, x_offset, row_coverage) {
                return false;
            }
            renderer.blend_row(
                extent.x + i32::from(x_offset),
                extent.y + i32::from(row),
                color,
                row_coverage,
            );
            x_offset += run as u16;
        }
    }
    true
}

struct ShadowMask {
    base: Rect,
    blur: i32,
    radius: i32,
}

impl AlphaMask for ShadowMask {
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]) {
        for (offset, alpha) in coverage.iter_mut().enumerate() {
            let px = x.saturating_add(i32::try_from(offset).unwrap_or(i32::MAX));
            *alpha = self.alpha(px, y);
        }
    }
}

impl ShadowMask {
    fn alpha(&self, x: i32, y: i32) -> u8 {
        let x0 = self.base.x;
        let y0 = self.base.y;
        let x1 = self.base.x + self.base.width - 1;
        let y1 = self.base.y + self.base.height - 1;
        let outside_dx = if x < x0 {
            x0 - x
        } else if x > x1 {
            x - x1
        } else {
            0
        };
        let outside_dy = if y < y0 {
            y0 - y
        } else if y > y1 {
            y - y1
        } else {
            0
        };
        let dist = outside_dx.max(outside_dy);
        if dist > self.blur {
            return 0;
        }

        let rect_alpha = if self.blur == 0 {
            255
        } else {
            (((self.blur + 1 - dist) * 255) / (self.blur + 1)) as u8
        };
        rect_alpha.min(self.rounded_corner_alpha(x, y))
    }

    fn rounded_corner_alpha(&self, x: i32, y: i32) -> u8 {
        let r = self
            .radius
            .max(0)
            .min(self.base.width / 2)
            .min(self.base.height / 2);
        if r <= 0 {
            return 255;
        }

        let cx = if x < self.base.x + r {
            self.base.x + r
        } else if x >= self.base.x + self.base.width - r {
            self.base.x + self.base.width - r - 1
        } else {
            return 255;
        };
        let cy = if y < self.base.y + r {
            self.base.y + r
        } else if y >= self.base.y + self.base.height - r {
            self.base.y + self.base.height - r - 1
        } else {
            return 255;
        };
        let dx = (x - cx).unsigned_abs();
        let dy = (y - cy).unsigned_abs();
        let dist_sq = dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy));
        let r_sq = (r as u32).saturating_mul(r as u32);
        if dist_sq <= r_sq { 255 } else { 0 }
    }
}

// ───────────────────────────────────────────────────────────────────────────
// ClipRenderer (REND initiative)
// ───────────────────────────────────────────────────────────────────────────

/// Nominal line height (pixels above the baseline anchor) used to gate
/// backend [`Renderer::draw_text`] calls against a clip region. See
/// `docs/concepts/REND-00-CONCEPTS.md` §5.4 for the guarantee scope.
pub const TEXT_NOMINAL_LINE_PX: i32 = 16;

/// Translating, clipping [`Renderer`] adapter (REND-00 §5).
///
/// Wraps any renderer, shifts every draw by `(dx, dy)` (content space →
/// screen space), and crops it to a screen-space clip rect before
/// forwarding. Containers that render children clipped to their own
/// bounds — [`ScrollView`] being the canonical example — construct one of
/// these around the incoming renderer in their `draw()`; backends need no
/// changes and cannot opt out by accident.
///
/// Per-method semantics (normative — REND-00 §5.4):
///
/// - [`fill_rect`](Renderer::fill_rect) / [`blend_rect`](Renderer::blend_rect):
///   forwarded as the translated intersection with the clip; dropped when
///   disjoint.
/// - [`draw_pixels`](Renderer::draw_pixels): only the visible row segments
///   are forwarded (per-row source slice) — cropping never reads outside
///   the source buffer.
/// - [`blend_row`](Renderer::blend_row): the coverage run is sliced to the
///   clip's horizontal span; rows outside the vertical span are dropped.
///   The AA primitives (`fill_obb_aa`, `fill_disc_aa`, `stroke_line_aa`,
///   `fill_arc_aa`), [`fill_masked`](Renderer::fill_masked),
///   [`fill_gradient`](Renderer::fill_gradient),
///   [`draw_shadow`](Renderer::draw_shadow), and [`submit`](Renderer::submit)
///   are deliberately NOT forwarded to the wrapped renderer: the trait
///   defaults route them through this adapter's clipped primitives. A
///   forwarding override would hand the call to a backend fast path that knows
///   nothing of the clip.
/// - [`draw_text`](Renderer::draw_text) (backend text — no extent
///   information): forwarded iff the nominal line box
///   ([`TEXT_NOMINAL_LINE_PX`] above the baseline) sits fully inside the
///   clip's vertical span and the anchor is inside the horizontal span;
///   otherwise dropped. No vertical bleed, but partially-visible lines
///   vanish rather than crop, and horizontal overflow is not cropped.
///   Per-pixel text (`bitmap_font` / `packed_font`) renders through
///   `fill_rect` and clips exactly on both axes — prefer it for content
///   that can straddle a viewport edge.
/// - [`draw_text_shaped`](Renderer::draw_text_shaped): routes through the
///   default shaped-text renderer, so glyph coverage is clipped by
///   [`blend_row`](Renderer::blend_row) and extent fallbacks are clipped by
///   [`blend_rect`](Renderer::blend_rect). The wrapped renderer's own
///   shaped-text fast path is deliberately not forwarded, so clipping cannot
///   be bypassed by an accelerated backend.
///
/// Nesting two `ClipRenderer`s composes by intersection with summed
/// offsets (REND-00 §5.5).
///
/// `clip` is in *screen* (wrapped-renderer) coordinates. Construction with
/// a degenerate clip is allowed; every draw is then dropped.
pub struct ClipRenderer<'a> {
    inner: &'a mut dyn Renderer,
    /// Translation applied to incoming draws (content space → screen).
    dx: i32,
    dy: i32,
    /// Screen-space clip region; `None` means degenerate (drop all).
    clip: Option<Rect>,
}

impl<'a> ClipRenderer<'a> {
    /// Wrap `inner`, clipping to the screen-space `clip` rect with no
    /// translation.
    pub fn new(inner: &'a mut dyn Renderer, clip: Rect) -> Self {
        Self::with_offset(inner, clip, 0, 0)
    }

    /// Wrap `inner`, translating incoming draws by `(dx, dy)` and clipping
    /// the result to the screen-space `clip` rect.
    pub fn with_offset(inner: &'a mut dyn Renderer, clip: Rect, dx: i32, dy: i32) -> Self {
        let clip = (clip.width > 0 && clip.height > 0).then_some(clip);
        Self {
            inner,
            dx,
            dy,
            clip,
        }
    }

    /// The screen-space clip rect, or `None` when degenerate.
    pub fn clip(&self) -> Option<Rect> {
        self.clip
    }
}

impl Renderer for ClipRenderer<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let Some(clip) = self.clip else { return };
        let moved = Rect {
            x: rect.x + self.dx,
            y: rect.y + self.dy,
            ..rect
        };
        if let Some(visible) = moved.intersect(clip) {
            self.inner.fill_rect(visible, color);
        }
    }

    fn blend_rect(&mut self, rect: Rect, color: Color) {
        let Some(clip) = self.clip else { return };
        let moved = Rect {
            x: rect.x + self.dx,
            y: rect.y + self.dy,
            ..rect
        };
        if let Some(visible) = moved.intersect(clip) {
            self.inner.blend_rect(visible, color);
        }
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        let Some(clip) = self.clip else { return };
        let (x, y) = (position.0 + self.dx, position.1 + self.dy);
        // Nominal-line-box gating (REND-00 §5.4): vertical containment of
        // the line above the baseline, horizontal containment of the
        // anchor. Drop rather than bleed.
        let line_top = y - TEXT_NOMINAL_LINE_PX;
        let inside_v = line_top >= clip.y && y <= clip.y + clip.height;
        let inside_h = x >= clip.x && x < clip.x + clip.width;
        if inside_v && inside_h {
            self.inner.draw_text((x, y), text, color);
        }
    }

    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        let Some(clip) = self.clip else { return };
        if width == 0 || height == 0 {
            return;
        }
        let dest = Rect {
            x: position.0 + self.dx,
            y: position.1 + self.dy,
            width: width as i32,
            height: height as i32,
        };
        let Some(visible) = dest.intersect(clip) else {
            return;
        };
        // Fast path: fully visible — forward unchanged.
        if visible == dest {
            self.inner
                .draw_pixels((dest.x, dest.y), pixels, width, height);
            return;
        }
        // Partial: forward each visible row's segment as a 1-row blit.
        let col0 = (visible.x - dest.x) as u32;
        let run = visible.width as u32;
        for row in 0..visible.height {
            let src_row = (visible.y - dest.y + row) as u32;
            let start = (src_row * width + col0) as usize;
            let Some(slice) = pixels.get(start..start + run as usize) else {
                return;
            };
            self.inner
                .draw_pixels((visible.x, visible.y + row), slice, run, 1);
        }
    }

    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        let Some(clip) = self.clip else { return };
        let (x, y) = (x + self.dx, y + self.dy);
        if y < clip.y || y >= clip.y + clip.height || coverage.is_empty() {
            return;
        }
        let row = Rect {
            x,
            y,
            width: coverage.len() as i32,
            height: 1,
        };
        let Some(visible) = row.intersect(clip) else {
            return;
        };
        let start = (visible.x - x) as usize;
        self.inner.blend_row(
            visible.x,
            y,
            color,
            &coverage[start..start + visible.width as usize],
        );
    }

    // fill_obb_aa / fill_disc_aa / stroke_line_aa / fill_arc_aa /
    // fill_masked / fill_gradient / draw_shadow / submit:
    // intentionally NOT overridden (REND-00 §5.3). The trait defaults
    // funnel them through `blend_row` / per-cmd dispatch on *this*
    // adapter, which clips. Forwarding them to `inner` would bypass the
    // clip on backends with hardware fast paths.
}
