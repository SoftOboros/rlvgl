//! Tests for the graphics-language layer: Cmd / CommandList / Recorder /
//! CommandSink. The load-bearing test is the **round-trip equivalence**:
//! a widget tree captured into a CommandList and replayed onto a fresh
//! renderer must produce a buffer bit-identical to direct rendering of
//! the same tree. That locks in the language layer as a pure capture +
//! replay mechanism — never a behavior change vs the direct path.

use rlvgl_core::cmd::{BlendMode, Cmd, CommandList, CommandSink, Recorder, RendererSink};
use rlvgl_core::raster::{Obb, PointF};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};

/// Minimal Renderer impl over an ARGB8888 byte buffer. Mirrors the
/// platform PixelsRenderer enough to be a meaningful round-trip target
/// without requiring rlvgl-platform as a test dep.
struct ArgbBuffer<'a> {
    buf: &'a mut [u8],
    width: usize,
    height: usize,
}

impl<'a> ArgbBuffer<'a> {
    fn new(buf: &'a mut [u8], width: usize, height: usize) -> Self {
        Self { buf, width, height }
    }

    fn put_blend(&mut self, x: i32, y: i32, color: Color, alpha_u8: u8) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let idx = (y as usize * self.width + x as usize) * 4;
        let a = alpha_u8 as u16;
        let inv = 255 - a;
        let bg_r = self.buf[idx] as u16;
        let bg_g = self.buf[idx + 1] as u16;
        let bg_b = self.buf[idx + 2] as u16;
        self.buf[idx] = ((color.0 as u16 * a + bg_r * inv) / 255) as u8;
        self.buf[idx + 1] = ((color.1 as u16 * a + bg_g * inv) / 255) as u8;
        self.buf[idx + 2] = ((color.2 as u16 * a + bg_b * inv) / 255) as u8;
        self.buf[idx + 3] = 0xff;
    }
}

impl Renderer for ArgbBuffer<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width).min(self.width as i32);
        let y1 = (rect.y + rect.height).min(self.height as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                let idx = (y as usize * self.width + x as usize) * 4;
                self.buf[idx] = color.0;
                self.buf[idx + 1] = color.1;
                self.buf[idx + 2] = color.2;
                self.buf[idx + 3] = 0xff;
            }
        }
    }

    fn draw_text(&mut self, _: (i32, i32), _: &str, _: Color) {}

    fn blend_rect(&mut self, rect: Rect, color: Color) {
        let alpha = color.3;
        if alpha == 0 {
            return;
        }
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width).min(self.width as i32);
        let y1 = (rect.y + rect.height).min(self.height as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                self.put_blend(x, y, color, alpha);
            }
        }
    }

    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        if color.3 == 0 {
            return;
        }
        let src_a = color.3 as u16;
        for (i, &cov) in coverage.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let a = ((src_a * cov as u16) / 255) as u8;
            if a == 0 {
                continue;
            }
            self.put_blend(x + i as i32, y, color, a);
        }
    }
}

fn fresh_buf(w: usize, h: usize) -> Vec<u8> {
    let mut buf = vec![0u8; w * h * 4];
    for px in buf.chunks_exact_mut(4) {
        px[3] = 0xff;
    }
    buf
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
    // Arc — quarter slice.
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
    // Solid fill.
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

#[test]
fn recorder_then_replay_matches_direct_render() {
    let w = 80usize;
    let h = 80usize;
    let mut buf_direct = fresh_buf(w, h);
    let mut buf_replay = fresh_buf(w, h);

    {
        let mut r = ArgbBuffer::new(&mut buf_direct, w, h);
        paint_test_scene(&mut r);
    }

    let mut list = CommandList::new();
    {
        let mut throwaway = fresh_buf(w, h);
        let mut sink = ArgbBuffer::new(&mut throwaway, w, h);
        let mut recorder = Recorder::new(&mut list, &mut sink);
        paint_test_scene(&mut recorder);
    }

    {
        let mut r = ArgbBuffer::new(&mut buf_replay, w, h);
        let mut rs = RendererSink::new(&mut r);
        rs.submit(&list);
    }

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
        "captured-then-replayed scene must be bit-identical to direct render; \
         {diffs} bytes differ (max delta {max_diff})"
    );
}

#[test]
fn recorder_captures_expected_cmd_count() {
    // paint_test_scene issues 1 OBB, 1 disc, 1 arc, 1 line, 1 fill, 1
    // blend = 6 structured cmds. (draw_text would be passthrough, but
    // the scene doesn't use it.)
    let mut list = CommandList::new();
    let mut throwaway = fresh_buf(80, 80);
    let mut sink = ArgbBuffer::new(&mut throwaway, 80, 80);
    let mut recorder = Recorder::new(&mut list, &mut sink);
    paint_test_scene(&mut recorder);
    assert_eq!(list.len(), 6, "scene should produce 6 structured cmds");
}

#[test]
fn cmd_aabb_returns_drawn_pixels_envelope() {
    // Spot-check: FillRect aabb is the rect; FillDisc aabb encloses the
    // disc with 1-px AA padding; FillObb aabb matches Obb::aabb.
    let rect_cmd = Cmd::FillRect {
        rect: Rect {
            x: 10,
            y: 20,
            width: 30,
            height: 40,
        },
        color: Color(0, 0, 0, 255),
        blend: BlendMode::Replace,
    };
    let bb = rect_cmd.aabb().expect("FillRect should have aabb");
    assert_eq!(bb.width, 30);
    assert_eq!(bb.height, 40);

    let disc_cmd = Cmd::FillDisc {
        center: PointF::new(50.0, 50.0),
        radius: 10.0,
        color: Color(0, 0, 0, 255),
        blend: BlendMode::SrcOver,
    };
    let bb = disc_cmd.aabb().expect("FillDisc should have aabb");
    // Conservative aabb: roughly 22x22 (radius+1 padding ×2 plus
    // floor/ceil margin). Just verify it covers the geometry envelope.
    assert!(
        bb.width >= 21 && bb.width <= 25 && bb.height >= 21 && bb.height <= 25,
        "disc aabb should be ~22 px square; got {bb:?}"
    );

    let obb_cmd = Cmd::FillObb {
        obb: Obb::axis_aligned(PointF::new(50.0, 50.0), 30.0, 4.0),
        color: Color(0, 0, 0, 255),
        blend: BlendMode::SrcOver,
    };
    let cmd_bb = obb_cmd.aabb().expect("FillObb should have aabb");
    let direct_bb = Obb::axis_aligned(PointF::new(50.0, 50.0), 30.0, 4.0).aabb();
    assert_eq!(cmd_bb, direct_bb, "FillObb aabb must match Obb::aabb");

    assert!(Cmd::Barrier.aabb().is_none());
    assert!(Cmd::SetClip { rect: None }.aabb().is_none());
}

#[test]
fn dirty_union_unions_all_drawing_cmds() {
    let mut list = CommandList::new();
    list.push(Cmd::FillRect {
        rect: Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
        color: Color(0, 0, 0, 255),
        blend: BlendMode::Replace,
    });
    list.push(Cmd::FillRect {
        rect: Rect {
            x: 50,
            y: 50,
            width: 10,
            height: 10,
        },
        color: Color(0, 0, 0, 255),
        blend: BlendMode::Replace,
    });
    list.push(Cmd::Barrier);
    list.push(Cmd::SetClip { rect: None });

    let union = list.dirty_union().expect("non-empty union");
    assert_eq!(union.x, 0);
    assert_eq!(union.y, 0);
    assert_eq!(union.width, 60);
    assert_eq!(union.height, 60);
}

#[test]
fn empty_list_has_no_dirty_union() {
    let list = CommandList::new();
    assert!(list.dirty_union().is_none());
    assert!(list.is_empty());
}

#[test]
fn list_with_only_markers_has_no_dirty_union() {
    let mut list = CommandList::new();
    list.push(Cmd::Barrier);
    list.push(Cmd::SetClip {
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        }),
    });
    assert!(list.dirty_union().is_none());
}
