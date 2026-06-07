//! Graphics-language layer: structured drawing commands as data, separable
//! from execution.
//!
//! The [`Renderer`] trait is a *drawing API* — verbs invoked one at a time.
//! [`Cmd`] + [`CommandList`] define a *graphics language* — a noun
//! vocabulary where each draw is a value the backend consumes. The shift
//! lets backends:
//!
//! - Inspect the whole frame before executing any of it (occlusion
//!   analysis, opaque-cmd skip, dirty-rect union).
//! - Coalesce adjacent compatible ops (sequential same-color fills,
//!   batched OBB rasterizations).
//! - Chain hardware transfers (configure DMA2D for cmd N+1 while N runs).
//! - Bin commands by tile for cache-resident rendering.
//! - Replay a recorded list to a different surface (e.g. snapshot tests,
//!   off-screen pre-render of static content).
//!
//! See [`Recorder`] for capturing an existing widget's [`Renderer`] calls
//! into a [`CommandList`], and [`RendererSink`] for the default-dispatch
//! adapter that any [`Renderer`] can be wrapped in to act as a
//! [`CommandSink`].
//!
//! # Lifetime model
//!
//! v1 [`Cmd`] is `Clone` (owns its data — heap-allocated `String` /
//! `Vec<Color>` for text and pixel ops). This trades alloc per
//! unstructured op for a simpler, lifetime-free API. Future arena-backed
//! variant can be added behind a feature flag if alloc cost becomes
//! material.

use alloc::string::String;
use alloc::vec::Vec;

use crate::raster::{Obb, PointF};
use crate::renderer::Renderer;
use crate::widget::{Color, Rect};

/// Composition mode for a draw command. Backends that have hardware
/// blend modes can branch on this; software backends may collapse Add /
/// Multiply down to per-pixel arithmetic.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlendMode {
    /// Replace destination pixels (alpha-ignoring fast path).
    Replace,
    /// Standard source-over with `color.alpha * coverage`.
    SrcOver,
    /// Additive blend: dst = clamp(dst + src * alpha).
    Add,
    /// Multiplicative blend: dst = dst * src (alpha modulates).
    Multiply,
}

/// A single drawing command. Z-order is list position in the
/// [`CommandList`] that contains it.
#[derive(Clone, Debug)]
pub enum Cmd {
    /// Filled axis-aligned rectangle.
    FillRect {
        /// Rectangle bounds.
        rect: Rect,
        /// Fill color.
        color: Color,
        /// Composition mode.
        blend: BlendMode,
    },
    /// Anti-aliased filled oriented bounding box (rotated rectangle).
    FillObb {
        /// Pre-computed OBB geometry.
        obb: Obb,
        /// Fill color.
        color: Color,
        /// Composition mode.
        blend: BlendMode,
    },
    /// Anti-aliased filled disc (filled circle).
    FillDisc {
        /// Sub-pixel center.
        center: PointF,
        /// Radius in pixels.
        radius: f32,
        /// Fill color.
        color: Color,
        /// Composition mode.
        blend: BlendMode,
    },
    /// Anti-aliased annular arc / pie slice. See
    /// [`crate::raster::rasterize_arc`] for the angle convention.
    FillArc {
        /// Sub-pixel center.
        center: PointF,
        /// Outer radius in pixels.
        r_outer: f32,
        /// Inner radius in pixels (0.0 for a pie slice).
        r_inner: f32,
        /// Cosine of the start ray direction.
        start_cos: f32,
        /// Sine of the start ray direction.
        start_sin: f32,
        /// Cosine of the end ray direction.
        end_cos: f32,
        /// Sine of the end ray direction.
        end_sin: f32,
        /// Signed angular extent in radians.
        extent: f32,
        /// Fill color.
        color: Color,
        /// Composition mode.
        blend: BlendMode,
    },
    /// Anti-aliased stroked line of given width (square caps).
    StrokeLine {
        /// Line start.
        a: PointF,
        /// Line end.
        b: PointF,
        /// Line width in pixels.
        width: f32,
        /// Stroke color.
        color: Color,
        /// Composition mode.
        blend: BlendMode,
    },
    /// Text draw — owned `String` so the cmd can outlive the original
    /// borrow. See module docs for the alloc trade-off.
    DrawText {
        /// Anchor position (baseline origin).
        position: (i32, i32),
        /// UTF-8 text.
        text: String,
        /// Text color.
        color: Color,
    },
    /// Pixel-buffer blit — owned `Vec<Color>` for the same reason as
    /// [`Cmd::DrawText`]'s string.
    DrawPixels {
        /// Top-left destination position.
        position: (i32, i32),
        /// Source pixels, row-major.
        pixels: Vec<Color>,
        /// Source width in pixels.
        width: u32,
        /// Source height in pixels.
        height: u32,
    },
    /// Clip stack marker. `Some(rect)` pushes a clip; `None` clears.
    /// Backends should bound subsequent draws to the active clip.
    SetClip {
        /// Clip rectangle, or `None` to clear.
        rect: Option<Rect>,
    },
    /// Backend flush boundary (compositor swap, telemetry checkpoint).
    /// Optimizing backends may not reorder cmds across a Barrier.
    Barrier,
}

impl Cmd {
    /// Dispatch this cmd onto a [`Renderer`], using the matching trait
    /// method for each variant. This is the default execution semantics
    /// — backends override [`CommandSink`] to specialize.
    pub fn dispatch_to<R: Renderer + ?Sized>(&self, r: &mut R) {
        match self {
            Cmd::FillRect { rect, color, blend } => match blend {
                BlendMode::Replace => r.fill_rect(*rect, *color),
                BlendMode::SrcOver => r.blend_rect(*rect, *color),
                // Add / Multiply have no Renderer-trait equivalent yet;
                // fall through to SrcOver for v1. Backends with native
                // support override CommandSink directly.
                BlendMode::Add | BlendMode::Multiply => r.blend_rect(*rect, *color),
            },
            Cmd::FillObb { obb, color, .. } => r.fill_obb_aa(*obb, *color),
            Cmd::FillDisc {
                center,
                radius,
                color,
                ..
            } => r.fill_disc_aa(*center, *radius, *color),
            Cmd::FillArc {
                center,
                r_outer,
                r_inner,
                start_cos,
                start_sin,
                end_cos,
                end_sin,
                extent,
                color,
                ..
            } => r.fill_arc_aa(
                *center, *r_outer, *r_inner, *start_cos, *start_sin, *end_cos, *end_sin, *extent,
                *color,
            ),
            Cmd::StrokeLine {
                a, b, width, color, ..
            } => r.stroke_line_aa(*a, *b, *width, *color),
            Cmd::DrawText {
                position,
                text,
                color,
            } => r.draw_text(*position, text, *color),
            Cmd::DrawPixels {
                position,
                pixels,
                width,
                height,
            } => r.draw_pixels(*position, pixels, *width, *height),
            // Clip and Barrier are advisory for default dispatch — the
            // base Renderer trait has no clip stack. Backends that
            // implement clipping override CommandSink::submit directly.
            Cmd::SetClip { .. } | Cmd::Barrier => {}
        }
    }

    /// AABB of pixels this command might write, in absolute framebuffer
    /// coords. Returns `None` for non-drawing markers (Barrier, SetClip).
    pub fn aabb(&self) -> Option<Rect> {
        match self {
            Cmd::FillRect { rect, .. } => Some(*rect),
            Cmd::FillObb { obb, .. } => Some(obb.aabb()),
            Cmd::FillDisc { center, radius, .. }
            | Cmd::FillArc {
                center,
                r_outer: radius,
                ..
            } => {
                let pad = radius + 1.0;
                Some(Rect {
                    x: (center.x - pad) as i32 - 1,
                    y: (center.y - pad) as i32 - 1,
                    width: (pad * 2.0) as i32 + 3,
                    height: (pad * 2.0) as i32 + 3,
                })
            }
            Cmd::StrokeLine { a, b, width, .. } => {
                let pad = width * 0.5 + 1.0;
                let x0 = (a.x.min(b.x) - pad) as i32 - 1;
                let y0 = (a.y.min(b.y) - pad) as i32 - 1;
                let x1 = (a.x.max(b.x) + pad) as i32 + 2;
                let y1 = (a.y.max(b.y) + pad) as i32 + 2;
                Some(Rect {
                    x: x0,
                    y: y0,
                    width: x1 - x0,
                    height: y1 - y0,
                })
            }
            Cmd::DrawText { .. } => None, // text bbox depends on font metrics
            Cmd::DrawPixels {
                position,
                width,
                height,
                ..
            } => Some(Rect {
                x: position.0,
                y: position.1,
                width: *width as i32,
                height: *height as i32,
            }),
            Cmd::SetClip { .. } | Cmd::Barrier => None,
        }
    }
}

/// Ordered list of drawing commands. Z-order is list position (later =
/// on top). Backends consume the list via [`CommandSink::submit`].
#[derive(Clone, Debug, Default)]
pub struct CommandList {
    cmds: Vec<Cmd>,
}

impl CommandList {
    /// Create an empty command list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a command. Z-order is list position.
    pub fn push(&mut self, cmd: Cmd) {
        self.cmds.push(cmd);
    }

    /// Number of commands in the list.
    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    /// Whether the list contains no commands.
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    /// Iterate over commands in z-order (back to front).
    pub fn iter(&self) -> core::slice::Iter<'_, Cmd> {
        self.cmds.iter()
    }

    /// Drop all commands. Reuses the internal allocation.
    pub fn clear(&mut self) {
        self.cmds.clear();
    }

    /// Replay every command on `r` via [`Cmd::dispatch_to`]. This is the
    /// "default-dispatch" execution semantics — backends that want
    /// optimization implement [`CommandSink`] directly on themselves.
    pub fn replay<R: Renderer + ?Sized>(&self, r: &mut R) {
        for cmd in &self.cmds {
            cmd.dispatch_to(r);
        }
    }

    /// Union AABB of all drawing commands' write regions, computed via
    /// [`Cmd::aabb`]. Returns `None` if the list contains no draw cmds
    /// or only cmds with unknown bboxes (text).
    pub fn dirty_union(&self) -> Option<Rect> {
        let mut union: Option<Rect> = None;
        for cmd in &self.cmds {
            if let Some(bb) = cmd.aabb() {
                union = Some(match union {
                    None => bb,
                    Some(u) => rect_union(u, bb),
                });
            }
        }
        union
    }
}

fn rect_union(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.width).max(b.x + b.width);
    let y1 = (a.y + a.height).max(b.y + b.height);
    Rect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    }
}

/// Backend trait for executing a [`CommandList`]. The blanket impl is
/// not provided because that would prevent backends from supplying
/// optimized implementations; instead, wrap any [`Renderer`] in
/// [`RendererSink`] for default-dispatch semantics, or implement
/// `CommandSink` directly for native batching.
pub trait CommandSink {
    /// Execute the entire command list. The list is consumed by
    /// reference — backends that need owned cmds should clone.
    fn submit(&mut self, list: &CommandList);
}

/// Default-dispatch adapter: wraps any [`Renderer`] and submits cmds by
/// calling the matching trait method per command. Use this when you
/// want command-list capture/replay without per-backend optimization.
pub struct RendererSink<'a, R: Renderer + ?Sized> {
    /// Inner renderer.
    pub inner: &'a mut R,
}

impl<'a, R: Renderer + ?Sized> RendererSink<'a, R> {
    /// Wrap a renderer. The result acts as a [`CommandSink`].
    pub fn new(inner: &'a mut R) -> Self {
        Self { inner }
    }
}

impl<R: Renderer + ?Sized> CommandSink for RendererSink<'_, R> {
    fn submit(&mut self, list: &CommandList) {
        list.replay(self.inner);
    }
}

/// `Renderer` adapter that records structured AA draws into a
/// [`CommandList`]. Unstructured ops (`draw_text`, `draw_pixels`) and
/// `blend_row` forward immediately to a passthrough renderer rather
/// than recording — see module docs for the lifetime/alloc rationale.
///
/// Typical usage:
///
/// ```ignore
/// let mut list = CommandList::new();
/// {
///     let mut sink = NullRenderer::default(); // or any "do nothing" renderer
///     let mut recorder = Recorder::new(&mut list, &mut sink);
///     widget.draw(&mut recorder);
/// }
/// // list now contains the structured draws
/// ```
pub struct Recorder<'a> {
    list: &'a mut CommandList,
    passthrough: &'a mut dyn Renderer,
}

impl<'a> Recorder<'a> {
    /// Create a recorder writing structured draws into `list` and
    /// forwarding unstructured ops to `passthrough`.
    pub fn new(list: &'a mut CommandList, passthrough: &'a mut dyn Renderer) -> Self {
        Self { list, passthrough }
    }
}

impl Renderer for Recorder<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.list.push(Cmd::FillRect {
            rect,
            color,
            blend: BlendMode::Replace,
        });
    }

    fn blend_rect(&mut self, rect: Rect, color: Color) {
        self.list.push(Cmd::FillRect {
            rect,
            color,
            blend: BlendMode::SrcOver,
        });
    }

    fn fill_obb_aa(&mut self, obb: Obb, color: Color) {
        self.list.push(Cmd::FillObb {
            obb,
            color,
            blend: BlendMode::SrcOver,
        });
    }

    fn fill_disc_aa(&mut self, center: PointF, radius: f32, color: Color) {
        self.list.push(Cmd::FillDisc {
            center,
            radius,
            color,
            blend: BlendMode::SrcOver,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_arc_aa(
        &mut self,
        center: PointF,
        r_outer: f32,
        r_inner: f32,
        start_cos: f32,
        start_sin: f32,
        end_cos: f32,
        end_sin: f32,
        extent: f32,
        color: Color,
    ) {
        self.list.push(Cmd::FillArc {
            center,
            r_outer,
            r_inner,
            start_cos,
            start_sin,
            end_cos,
            end_sin,
            extent,
            color,
            blend: BlendMode::SrcOver,
        });
    }

    fn stroke_line_aa(&mut self, a: PointF, b: PointF, width: f32, color: Color) {
        self.list.push(Cmd::StrokeLine {
            a,
            b,
            width,
            color,
            blend: BlendMode::SrcOver,
        });
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        // Unstructured: forward immediately. The recorded list misses
        // this op; document the limitation. v2 may add owned-string
        // capture if profiling shows it matters.
        self.passthrough.draw_text(position, text, color);
    }

    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        // Same passthrough rationale as draw_text.
        self.passthrough
            .draw_pixels(position, pixels, width, height);
    }

    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        // blend_row is the AA inner-loop primitive. Forward to passthrough
        // since recording per-row coverage spans would explode the cmd
        // count and defeat the language layer's batching purpose. Higher
        // -level methods (fill_obb_aa etc.) record correctly above.
        self.passthrough.blend_row(x, y, color, coverage);
    }
}
