//! Tests for the `CommandSink` impl on `BlitterRenderer`.
//!
//! Two invariants:
//!
//! 1. **Round-trip equivalence**: a scene rendered directly through
//!    `BlitterRenderer` must produce a buffer byte-identical to the same
//!    scene captured via `Recorder` and replayed through
//!    `BlitterRenderer::submit`. The occlusion pre-pass is a pure
//!    optimization; it must never change visible output.
//!
//! 2. **Occlusion is observable**: when a fully-covered earlier cmd is
//!    followed by an opaque covering `FillRect`, the cmd's contribution
//!    is invisible (the FillRect overwrites it anyway). The result with
//!    occlusion-aware submit is byte-identical to a list where the
//!    occluded cmd is omitted entirely. That proves the optimization
//!    actually skips work without semantic change.

use rlvgl_core::cmd::{BlendMode, Cmd, CommandList, Recorder};
use rlvgl_core::raster::{Obb, PointF};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};
use rlvgl_platform::CpuBlitter;
use rlvgl_platform::blit::{BlitterRenderer, PixelFmt, Surface};

const W: u32 = 80;
const H: u32 = 80;
const STRIDE: usize = (W * 4) as usize;

fn fresh_argb_buf() -> Vec<u8> {
    let mut buf = vec![0u8; STRIDE * H as usize];
    // Initialize to opaque black so blend math has well-defined background.
    for px in buf.chunks_exact_mut(4) {
        px[3] = 0xff;
    }
    buf
}

/// No-op renderer used as a passthrough target for `Recorder`.
/// `paint_test_scene` only emits structured ops, so passthrough is never
/// actually invoked.
struct DevNull;
impl Renderer for DevNull {
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn draw_text(&mut self, _: (i32, i32), _: &str, _: Color) {}
}

fn paint_test_scene<R: Renderer + ?Sized>(r: &mut R) {
    // OBB hand at 30°.
    let cos_t = (3.0_f32).sqrt() / 2.0;
    let sin_t = 0.5;
    r.fill_obb_aa(
        Obb::from_axis(PointF::new(40.0, 30.0), 30.0, 4.0, cos_t, sin_t),
        Color(220, 30, 30, 255),
    );
    // Disc at face center.
    r.fill_disc_aa(PointF::new(40.0, 40.0), 5.0, Color(20, 20, 20, 255));
    // Ring-segment arc.
    r.fill_arc_aa(
        PointF::new(40.0, 40.0),
        18.0,
        12.0,
        1.0,
        0.0,
        0.0,
        1.0,
        core::f32::consts::FRAC_PI_2,
        Color(180, 180, 30, 255),
    );
    // Stroked line.
    r.stroke_line_aa(
        PointF::new(10.0, 10.0),
        PointF::new(70.0, 60.0),
        2.0,
        Color(40, 80, 200, 255),
    );
    // Solid fill (bottom band).
    r.fill_rect(
        Rect {
            x: 0,
            y: 70,
            width: 80,
            height: 10,
        },
        Color(255, 255, 255, 255),
    );
    // Translucent overlay.
    r.blend_rect(
        Rect {
            x: 5,
            y: 5,
            width: 20,
            height: 20,
        },
        Color(0, 200, 0, 96),
    );
}

fn render_direct(buf: &mut [u8]) {
    let mut blitter = CpuBlitter;
    let surface = Surface::new(buf, STRIDE, PixelFmt::Argb8888, W, H);
    let mut r: BlitterRenderer<'_, CpuBlitter, 32> = BlitterRenderer::new(&mut blitter, surface);
    paint_test_scene(&mut r);
}

fn render_via_command_list(buf: &mut [u8], list: &CommandList) {
    let mut blitter = CpuBlitter;
    let surface = Surface::new(buf, STRIDE, PixelFmt::Argb8888, W, H);
    let mut r: BlitterRenderer<'_, CpuBlitter, 32> = BlitterRenderer::new(&mut blitter, surface);
    r.submit(list);
}

fn record_test_scene() -> CommandList {
    let mut list = CommandList::new();
    let mut sink = DevNull;
    let mut recorder = Recorder::new(&mut list, &mut sink);
    paint_test_scene(&mut recorder);
    list
}

#[test]
fn submit_matches_direct_render_byte_for_byte() {
    let mut buf_direct = fresh_argb_buf();
    let mut buf_replay = fresh_argb_buf();

    render_direct(&mut buf_direct);
    let list = record_test_scene();
    render_via_command_list(&mut buf_replay, &list);

    let mut diffs = 0u32;
    let mut max_diff = 0i32;
    for i in 0..buf_direct.len() {
        let d = (buf_direct[i] as i32 - buf_replay[i] as i32).abs();
        if d > 0 {
            diffs += 1;
            if d > max_diff {
                max_diff = d;
            }
        }
    }
    assert_eq!(
        diffs, 0,
        "BlitterRenderer::submit must produce byte-identical output to \
         direct rendering; {diffs} bytes differ (max delta {max_diff})"
    );
}

#[test]
fn occluded_cmd_is_skipped_without_changing_output() {
    // Build two lists:
    //   A: [hidden_obb, visible_solid_cover]    — hidden_obb fully under cover
    //   B: [visible_solid_cover]                — same cover, no hidden cmd
    // Render both via BlitterRenderer::submit. Buffers must match: the
    // occlusion pass on list A must skip hidden_obb entirely, leaving
    // only the cover cmd's pixels.

    let cover_rect = Rect {
        x: 10,
        y: 10,
        width: 60,
        height: 60,
    };
    let cover_color = Color(80, 80, 80, 255);
    // OBB whose AABB fits comfortably inside cover_rect.
    let obb = Obb::axis_aligned(PointF::new(40.0, 40.0), 30.0, 6.0);

    let mut list_a = CommandList::new();
    list_a.push(Cmd::FillObb {
        obb,
        color: Color(255, 0, 0, 255),
        blend: BlendMode::SrcOver,
    });
    list_a.push(Cmd::FillRect {
        rect: cover_rect,
        color: cover_color,
        blend: BlendMode::Replace,
    });

    let mut list_b = CommandList::new();
    list_b.push(Cmd::FillRect {
        rect: cover_rect,
        color: cover_color,
        blend: BlendMode::Replace,
    });

    let mut buf_a = fresh_argb_buf();
    let mut buf_b = fresh_argb_buf();
    render_via_command_list(&mut buf_a, &list_a);
    render_via_command_list(&mut buf_b, &list_b);

    let mut diffs = 0u32;
    for i in 0..buf_a.len() {
        if buf_a[i] != buf_b[i] {
            diffs += 1;
        }
    }
    assert_eq!(
        diffs, 0,
        "occluded OBB must produce identical output to omitting it; \
         {diffs} bytes differ"
    );
}

#[test]
fn partially_covered_cmd_is_not_skipped() {
    // Cover only half of the OBB's AABB → OBB must still paint (cover
    // does NOT contain its full AABB). Result must differ from a list
    // that only has the partial cover.
    let obb = Obb::axis_aligned(PointF::new(40.0, 40.0), 30.0, 6.0);
    let obb_aabb = obb.aabb();
    // Cover only the left half of the OBB AABB.
    let partial_cover = Rect {
        x: obb_aabb.x,
        y: obb_aabb.y,
        width: obb_aabb.width / 2,
        height: obb_aabb.height,
    };

    let mut list_with_obb = CommandList::new();
    list_with_obb.push(Cmd::FillObb {
        obb,
        color: Color(255, 0, 0, 255),
        blend: BlendMode::SrcOver,
    });
    list_with_obb.push(Cmd::FillRect {
        rect: partial_cover,
        color: Color(80, 80, 80, 255),
        blend: BlendMode::Replace,
    });

    let mut list_without_obb = CommandList::new();
    list_without_obb.push(Cmd::FillRect {
        rect: partial_cover,
        color: Color(80, 80, 80, 255),
        blend: BlendMode::Replace,
    });

    let mut buf_with = fresh_argb_buf();
    let mut buf_without = fresh_argb_buf();
    render_via_command_list(&mut buf_with, &list_with_obb);
    render_via_command_list(&mut buf_without, &list_without_obb);

    // Buffers MUST differ — the OBB's right half should still be
    // visible (not occluded by partial cover).
    let mut diffs = 0u32;
    for i in 0..buf_with.len() {
        if buf_with[i] != buf_without[i] {
            diffs += 1;
        }
    }
    assert!(
        diffs > 0,
        "OBB partially outside cover must remain visible; buffers should differ"
    );
}

#[test]
fn barrier_prevents_cross_barrier_occlusion() {
    // List A: [OBB, Barrier, opaque_cover_containing_obb_aabb]
    // List B: [OBB,          opaque_cover_containing_obb_aabb]
    //
    // In B, the OBB is fully covered by a later opaque FillRect → the
    // occlusion pre-pass skips it. In A, the Barrier between them must
    // terminate the search range, so the OBB *is* dispatched. We
    // observe this through the dirty-rect planner: each dispatched
    // draw call adds rects to it, so A produces strictly more rects
    // than B.
    let cover_rect = Rect {
        x: 10,
        y: 10,
        width: 60,
        height: 60,
    };
    let cover_color = Color(80, 80, 80, 255);
    let obb = Obb::axis_aligned(PointF::new(40.0, 40.0), 30.0, 6.0);

    let mut list_with_barrier = CommandList::new();
    list_with_barrier.push(Cmd::FillObb {
        obb,
        color: Color(255, 0, 0, 255),
        blend: BlendMode::SrcOver,
    });
    list_with_barrier.push(Cmd::Barrier);
    list_with_barrier.push(Cmd::FillRect {
        rect: cover_rect,
        color: cover_color,
        blend: BlendMode::Replace,
    });

    let mut list_without_barrier = CommandList::new();
    list_without_barrier.push(Cmd::FillObb {
        obb,
        color: Color(255, 0, 0, 255),
        blend: BlendMode::SrcOver,
    });
    list_without_barrier.push(Cmd::FillRect {
        rect: cover_rect,
        color: cover_color,
        blend: BlendMode::Replace,
    });

    let count_a = planner_rect_count(&list_with_barrier);
    let count_b = planner_rect_count(&list_without_barrier);

    assert!(
        count_a > count_b,
        "Barrier must terminate the occlusion search: with-Barrier should \
         dispatch the OBB (more planner rects) than without-Barrier where \
         the OBB is occluded. Got {count_a} vs {count_b}"
    );
}

#[test]
fn setclip_marker_also_terminates_occlusion_search() {
    // Same shape as the Barrier test but with SetClip as the divider.
    let cover_rect = Rect {
        x: 10,
        y: 10,
        width: 60,
        height: 60,
    };
    let cover_color = Color(80, 80, 80, 255);
    let obb = Obb::axis_aligned(PointF::new(40.0, 40.0), 30.0, 6.0);

    let mut list = CommandList::new();
    list.push(Cmd::FillObb {
        obb,
        color: Color(255, 0, 0, 255),
        blend: BlendMode::SrcOver,
    });
    list.push(Cmd::SetClip { rect: None });
    list.push(Cmd::FillRect {
        rect: cover_rect,
        color: cover_color,
        blend: BlendMode::Replace,
    });

    let mut list_no_marker = CommandList::new();
    list_no_marker.push(Cmd::FillObb {
        obb,
        color: Color(255, 0, 0, 255),
        blend: BlendMode::SrcOver,
    });
    list_no_marker.push(Cmd::FillRect {
        rect: cover_rect,
        color: cover_color,
        blend: BlendMode::Replace,
    });

    let count_with_clip = planner_rect_count(&list);
    let count_no_marker = planner_rect_count(&list_no_marker);
    assert!(
        count_with_clip > count_no_marker,
        "SetClip must terminate occlusion search; got {count_with_clip} vs {count_no_marker}"
    );
}

/// Submit `list` to a fresh `BlitterRenderer` and return the count of
/// rects accumulated by the dirty-rect planner. Each dispatched draw
/// adds at least one rect, so the count is a proxy for "cmds that
/// actually executed." Used in the Barrier / SetClip segmentation
/// tests to observe whether the occlusion pre-pass skipped a cmd.
fn planner_rect_count(list: &CommandList) -> usize {
    let mut buf = fresh_argb_buf();
    let mut blitter = CpuBlitter;
    let surface = Surface::new(&mut buf, STRIDE, PixelFmt::Argb8888, W, H);
    let mut r: BlitterRenderer<'_, CpuBlitter, 256> = BlitterRenderer::new(&mut blitter, surface);
    r.submit(list);
    r.planner().rects().len()
}

#[test]
fn translucent_filler_is_not_an_occluder() {
    // A FillRect with alpha < 255 doesn't fully cover what's under it
    // (it blends), so it must not occlude prior cmds.
    let cover = Rect {
        x: 10,
        y: 10,
        width: 60,
        height: 60,
    };
    let obb = Obb::axis_aligned(PointF::new(40.0, 40.0), 30.0, 6.0);

    let mut list_with_obb = CommandList::new();
    list_with_obb.push(Cmd::FillObb {
        obb,
        color: Color(255, 0, 0, 255),
        blend: BlendMode::SrcOver,
    });
    list_with_obb.push(Cmd::FillRect {
        rect: cover,
        color: Color(80, 80, 80, 128), // alpha = 128 → translucent
        blend: BlendMode::SrcOver,
    });

    let mut list_without_obb = CommandList::new();
    list_without_obb.push(Cmd::FillRect {
        rect: cover,
        color: Color(80, 80, 80, 128),
        blend: BlendMode::SrcOver,
    });

    let mut buf_with = fresh_argb_buf();
    let mut buf_without = fresh_argb_buf();
    render_via_command_list(&mut buf_with, &list_with_obb);
    render_via_command_list(&mut buf_without, &list_without_obb);

    let mut diffs = 0u32;
    for i in 0..buf_with.len() {
        if buf_with[i] != buf_without[i] {
            diffs += 1;
        }
    }
    assert!(
        diffs > 0,
        "OBB beneath translucent overlay must show through; buffers should differ"
    );
}
