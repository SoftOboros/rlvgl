// SPDX-License-Identifier: MIT
//! Platform-level effect primitives.
//!
//! An [`Effect`] is a stateful, cooperative renderer that contributes
//! pixels to a target [`Surface`] over one or more frames. Concrete
//! effect implementations — star crawls, audio scopes, event overlays —
//! live in higher-level crates. This module fixes the lifecycle contract
//! a BSP needs to drive one (`activate` / `tick` / `draw` /
//! `deactivate`), the [`EffectSink`] op vocabulary an effect emits its
//! frame description through, and the shared parameter shape for
//! scroll-style crawls.
//!
//! ## Why a sink, not a blitter
//!
//! An earlier shape of this trait took `paint<B: Blitter>(&mut self,
//! blitter, dst)`. That binds the effect to a specific rendering path
//! and makes it impossible to batch or reorder the per-frame work.
//! [`Effect::draw`] instead takes `&mut dyn EffectSink`: the effect
//! *describes* its frame as a sequence of ops ([`EffectSink::fill`],
//! [`EffectSink::blit`], [`EffectSink::blend_a8_row`]) and the sink
//! decides how to execute them. The shipped sink
//! [`BlitterSink`] flushes each op synchronously through any
//! [`Blitter`] (software CPU loops, DMA2D, WGPU); a future
//! `Dma2dBatchSink` can accumulate ops and fuse adjacent fills into a
//! single DMA2D transaction, or spread work across ERIF back porches.
//! None of that leaks into the effect itself — this is the same
//! command-list / backend split GStreamer uses for its graph execution.
//!
//! The trait is object-safe ([`Effect::draw`] uses `&mut dyn
//! EffectSink`), so hosts can store `Box<dyn Effect>` when they want
//! polymorphism across effect flavours. For CPU callers who already
//! hold a [`Blitter`], the [`EffectExt::paint`] convenience wraps the
//! blitter in a [`BlitterSink`] and calls `draw` in one shot.
//!
//! See [`CrawlParams`] for the shared tuning knobs that any text-style
//! crawl (Star Wars crawl, news ticker, credits roll) can consume.

use crate::blit::{Blitter, PixelFmt, Rect, Surface};

/// Render-op vocabulary an [`Effect`] emits into each frame.
///
/// This is the command-list boundary between "what to draw" (the
/// effect) and "how to draw it" (the sink / backend). Every op is a
/// self-contained description — the sink can execute it immediately
/// (see [`BlitterSink`]) or buffer and fuse it with subsequent ops
/// before dispatching to hardware.
///
/// The vocabulary was chosen to cover the existing star-crawl paint
/// path (jumbo background blit + per-row A8 alpha blend for text),
/// news-ticker-style patterns (scrolling blit + glyph row blend), and
/// audio-scope oscillogram patterns (fills + single-row blends).
/// Additional ops can be added as new effect flavours appear — keep
/// the additions orthogonal, not "whatever this one effect needs".
pub trait EffectSink {
    /// Destination width in pixels as the sink sees it. Effects use
    /// this to size per-frame work (number of rows to blend, clipping).
    fn target_width(&self) -> u32;

    /// Destination height in pixels as the sink sees it.
    fn target_height(&self) -> u32;

    /// Destination pixel format. Effects that only support a subset
    /// (e.g. ARGB8888) can bail early without emitting ops.
    fn target_format(&self) -> PixelFmt;

    /// Fill `rect` on the destination with a solid ARGB8888 colour.
    fn fill(&mut self, rect: Rect, color: u32);

    /// Copy `src_area` from `src` to `dst_pos` on the destination.
    ///
    /// Source and destination formats must match what the underlying
    /// backend supports; the stock [`BlitterSink`] forwards to the
    /// Blitter's format coverage, DMA2D handles ARGB8888 and RGB565.
    fn blit(&mut self, src: &Surface<'_>, src_area: Rect, dst_pos: (i32, i32));

    /// Alpha-blend a single A8 row onto the destination with a fixed
    /// foreground colour.
    ///
    /// `alphas` is a contiguous slice of A8 alpha bytes (one per
    /// pixel). `dst_pos` is the top-left destination corner. Semantics
    /// match DMA2D's A4/A8 foreground + fixed-colour + destination-fetch
    /// blend mode: `out = (fg_rgb * alpha + bg_rgb * (255 - alpha)) / 255`
    /// with `out_alpha = 255`.
    ///
    /// Rows come up frequently in text crawls after a perspective-taper
    /// FIR resample — the effect writes the resampled row into a scratch
    /// buffer and hands the slice to the sink. Multi-row A8 blends and
    /// per-pixel colour A8 blends are out of scope for now; add them
    /// when a second effect needs them.
    fn blend_a8_row(&mut self, alphas: &[u8], dst_pos: (i32, i32), fg_color: u32);
}

/// Offset + bounded wrapper around another [`EffectSink`].
///
/// All ops are translated by `offset` before being forwarded to the
/// inner sink, and [`target_width`](EffectSink::target_width) /
/// [`target_height`](EffectSink::target_height) report the clipped
/// sub-region. Used by widget-tree hosts (e.g.
/// `rlvgl_widgets::motion::crawl::CrawlWindow`) to place an effect at a
/// specific screen location without rewriting every op coordinate
/// inside the effect. No byte-level clipping — the effect is expected
/// to respect the reported target dimensions; the wrapper doesn't
/// intersect rectangles with the sub-region.
pub struct SubSink<'a> {
    inner: &'a mut dyn EffectSink,
    offset: (i32, i32),
    width: u32,
    height: u32,
}

impl<'a> SubSink<'a> {
    /// Build a sub-sink over `inner` at `offset` with `(width,
    /// height)` as the reported target size.
    #[inline]
    pub fn new(inner: &'a mut dyn EffectSink, offset: (i32, i32), width: u32, height: u32) -> Self {
        Self {
            inner,
            offset,
            width,
            height,
        }
    }
}

impl<'a> EffectSink for SubSink<'a> {
    fn target_width(&self) -> u32 {
        self.width
    }
    fn target_height(&self) -> u32 {
        self.height
    }
    fn target_format(&self) -> PixelFmt {
        self.inner.target_format()
    }
    fn fill(&mut self, rect: Rect, color: u32) {
        let rect = Rect {
            x: rect.x + self.offset.0,
            y: rect.y + self.offset.1,
            w: rect.w,
            h: rect.h,
        };
        self.inner.fill(rect, color);
    }
    fn blit(&mut self, src: &Surface<'_>, src_area: Rect, dst_pos: (i32, i32)) {
        self.inner.blit(
            src,
            src_area,
            (dst_pos.0 + self.offset.0, dst_pos.1 + self.offset.1),
        );
    }
    fn blend_a8_row(&mut self, alphas: &[u8], dst_pos: (i32, i32), fg_color: u32) {
        self.inner.blend_a8_row(
            alphas,
            (dst_pos.0 + self.offset.0, dst_pos.1 + self.offset.1),
            fg_color,
        );
    }
}

/// Lifecycle contract for a platform-level visual effect.
///
/// Boards (BSPs) drive effects through this trait: start one with
/// [`Effect::activate`], advance per-frame state with [`Effect::tick`],
/// describe the current frame with [`Effect::draw`] into an
/// [`EffectSink`], and tear down with [`Effect::deactivate`].
/// [`Effect::is_active`] gates whether the host needs to keep calling
/// `tick` / `draw`.
///
/// The trait is object-safe (no generics on any required method) so
/// hosts can polymorphism-dispatch over `Box<dyn Effect>` when they
/// want a pool of heterogeneous effects. The generic CPU convenience
/// [`EffectExt::paint`] lives on a supertrait to preserve that.
pub trait Effect {
    /// Returns `true` while this effect is contributing work.
    ///
    /// The effect is expected to drop to `false` of its own accord when
    /// its animation cycle finishes (for example, a crawl scrolling
    /// past its end). Hosts can also drive the flag down explicitly
    /// via [`Effect::deactivate`].
    fn is_active(&self) -> bool;

    /// Prepare internal buffers and start a fresh animation cycle.
    ///
    /// Re-activating an already-active effect resets it to the start
    /// of a new cycle.
    fn activate(&mut self);

    /// Stop animating.
    ///
    /// Subsequent [`Effect::tick`] calls are a no-op until
    /// [`Effect::activate`] is called again.
    fn deactivate(&mut self);

    /// Advance per-frame animation state by exactly one frame.
    ///
    /// Hosts pace this from their frame clock (wall-clock or VSYNC) so
    /// that visible motion tracks the declared rate regardless of CPU
    /// load variations.
    fn tick(&mut self);

    /// Emit the current frame's render ops into `sink`.
    ///
    /// The effect owns no framebuffer memory of its own. The sink owns
    /// the destination surface (directly, in the case of
    /// [`BlitterSink`]) or accumulates ops for later flushing (a
    /// future `Dma2dBatchSink`).
    fn draw(&mut self, sink: &mut dyn EffectSink);
}

/// CPU-convenience extension trait: bridge a [`Blitter`] into a
/// throwaway [`BlitterSink`] and call [`Effect::draw`] in one step.
///
/// The generic `paint<B: Blitter>` method is kept here rather than on
/// [`Effect`] itself so that [`Effect`] stays object-safe. Any type
/// that implements [`Effect`] automatically gets this method — hosts
/// keep writing `effect.paint(&mut blitter, &mut dst)` exactly like
/// before.
pub trait EffectExt: Effect {
    /// Paint the current frame into `dst` using `blitter`.
    ///
    /// Shorthand for constructing a [`BlitterSink`] over `(blitter,
    /// dst)` and calling [`Effect::draw`].
    fn paint<B: Blitter>(&mut self, blitter: &mut B, dst: &mut Surface<'_>) {
        let mut sink = BlitterSink::new(blitter, dst);
        self.draw(&mut sink);
    }
}

impl<T: Effect + ?Sized> EffectExt for T {}

/// Synchronous [`EffectSink`] that executes each op through a
/// [`Blitter`] against a caller-supplied destination [`Surface`].
///
/// This is the "CPU path" (and the fallback path on every target): no
/// buffering, no reordering — each `fill` / `blit` / `blend_a8_row`
/// call forwards straight to the blitter or runs an inline ARGB8888
/// scanline blend. It's the sink the BBB Linux build uses today, and
/// it's the right sink on hardware when you want the crawl engine to
/// drive DMA2D via the [`Blitter`] surface (each DMA2D op is
/// start+wait so the semantics stay synchronous).
pub struct BlitterSink<'a, 'b, B: Blitter> {
    blitter: &'a mut B,
    dst: &'a mut Surface<'b>,
}

impl<'a, 'b, B: Blitter> BlitterSink<'a, 'b, B> {
    /// Build a new sink wrapping `blitter` and borrowing `dst` for the
    /// lifetime of the sink. The sink has no lifetime beyond a single
    /// [`Effect::draw`] call; callers rebuild one each frame.
    #[inline]
    pub fn new(blitter: &'a mut B, dst: &'a mut Surface<'b>) -> Self {
        Self { blitter, dst }
    }
}

impl<'a, 'b, B: Blitter> EffectSink for BlitterSink<'a, 'b, B> {
    fn target_width(&self) -> u32 {
        self.dst.width
    }
    fn target_height(&self) -> u32 {
        self.dst.height
    }
    fn target_format(&self) -> PixelFmt {
        self.dst.format
    }
    fn fill(&mut self, rect: Rect, color: u32) {
        self.blitter.fill(self.dst, rect, color);
    }

    fn blit(&mut self, src: &Surface<'_>, src_area: Rect, dst_pos: (i32, i32)) {
        self.blitter.blit(src, src_area, self.dst, dst_pos);
    }

    fn blend_a8_row(&mut self, alphas: &[u8], dst_pos: (i32, i32), fg_color: u32) {
        blend_a8_row_inline(self.dst, alphas, dst_pos, fg_color);
    }
}

/// CPU implementation of [`EffectSink::blend_a8_row`].
///
/// Exposed as a free function so alternate sinks (tests, future
/// compositors) can reuse the exact byte-for-byte blend without
/// pulling in the full `BlitterSink` type. The destination format
/// must be [`PixelFmt::Argb8888`]; A8 sources against other formats
/// are silently dropped because the blend math assumes 4 bytes per
/// destination pixel in little-endian BGRA byte order.
pub fn blend_a8_row_inline(
    dst: &mut Surface<'_>,
    alphas: &[u8],
    dst_pos: (i32, i32),
    fg_color: u32,
) {
    if dst.format != PixelFmt::Argb8888 {
        return;
    }
    let (dx, dy) = dst_pos;
    if dy < 0 || (dy as u32) >= dst.height {
        return;
    }
    let dst_stride = dst.stride;
    let dst_w = dst.width as i32;
    let row_start = (dy as usize) * dst_stride;
    let fg_r = ((fg_color >> 16) & 0xFF) as u32;
    let fg_g = ((fg_color >> 8) & 0xFF) as u32;
    let fg_b = (fg_color & 0xFF) as u32;
    for (i, &alpha_byte) in alphas.iter().enumerate() {
        let alpha = alpha_byte as u32;
        if alpha == 0 {
            continue;
        }
        let x = dx + i as i32;
        if x < 0 || x >= dst_w {
            continue;
        }
        let dst_off = row_start + (x as usize) * 4;
        if dst_off + 4 > dst.buf.len() {
            break;
        }
        let bg_b = dst.buf[dst_off] as u32;
        let bg_g = dst.buf[dst_off + 1] as u32;
        let bg_r = dst.buf[dst_off + 2] as u32;
        let inv = 255 - alpha;
        dst.buf[dst_off] = ((fg_b * alpha + bg_b * inv) / 255) as u8;
        dst.buf[dst_off + 1] = ((fg_g * alpha + bg_g * inv) / 255) as u8;
        dst.buf[dst_off + 2] = ((fg_r * alpha + bg_r * inv) / 255) as u8;
        dst.buf[dst_off + 3] = 0xFF;
    }
}

/// Common tuning parameters for a text-style scroll crawl.
///
/// Concrete presets (the disco-demo star crawl, a news ticker, …) fill
/// these in. The platform crate doesn't mandate a specific rendering
/// style — only a shared shape so BSPs can treat crawls interchangeably
/// and so rate / spacing / perspective tuning is lifted out of ad-hoc
/// per-board constants.
///
/// The `viewport_w` / `viewport_h` fields describe the target rendering
/// region — what the effect paints into — and are used by callers to
/// size jumbo / text-source / scanline buffers before handing them to
/// the factory. They are not consumed by the effect itself.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CrawlParams {
    /// Target viewport width in pixels.
    pub viewport_w: u32,
    /// Target viewport height in pixels.
    pub viewport_h: u32,
    /// Host frame rate in Hz. Effects use this together with
    /// `pixels_per_sec` to derive the per-frame scroll increment.
    pub frame_hz: u32,
    /// Declared wall-clock scroll speed, in pixels per second.
    pub pixels_per_sec: u32,
    /// Inter-line spacing in pixels for pre-rendered text.
    pub line_spacing_px: u16,
    /// Text colour in ARGB8888 byte order.
    pub text_color_argb: u32,
    /// Divider applied to the text scroll accumulator when sampling
    /// the background. `1` locks the background to the text; higher
    /// values create a parallax effect.
    pub background_scroll_divisor: u16,
    /// Projected text width at the top edge of the viewport.
    pub perspective_top_width: u16,
    /// Projected text width at the bottom edge of the viewport.
    pub perspective_bottom_width: u16,
}

impl CrawlParams {
    /// Disco-demo star-crawl defaults: 40 px/s upward scroll, 36 px
    /// line spacing, 360→600 px perspective taper, canonical yellow
    /// text, background drifting at 1/3 the text speed.
    ///
    /// Matches the STM32H747I-DISCO hardware star crawl and the BBB
    /// Linux port. Hosts that want the sim-style looser spacing can
    /// mutate `line_spacing_px` after construction.
    pub const fn star_crawl_disco(viewport_w: u32, viewport_h: u32, frame_hz: u32) -> Self {
        Self {
            viewport_w,
            viewport_h,
            frame_hz,
            pixels_per_sec: 40,
            line_spacing_px: 36,
            text_color_argb: 0xFFFF_D700,
            background_scroll_divisor: 3,
            perspective_top_width: 360,
            perspective_bottom_width: 600,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_crawl_disco_matches_hardware_baseline() {
        let p = CrawlParams::star_crawl_disco(720, 480, 57);
        assert_eq!(p.viewport_w, 720);
        assert_eq!(p.viewport_h, 480);
        assert_eq!(p.frame_hz, 57);
        assert_eq!(p.pixels_per_sec, 40);
        assert_eq!(p.line_spacing_px, 36);
        assert_eq!(p.text_color_argb, 0xFFFF_D700);
        assert_eq!(p.background_scroll_divisor, 3);
        assert_eq!(p.perspective_top_width, 360);
        assert_eq!(p.perspective_bottom_width, 600);
    }

    #[test]
    fn crawl_params_is_copy() {
        let a = CrawlParams::star_crawl_disco(720, 480, 57);
        let b = a;
        assert_eq!(a, b);
    }
}
