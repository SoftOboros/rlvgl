//! Host-side integration test for the typed DMA2D submission API.
//!
//! Exercises the full lifecycle of `FrameBuffer` → `BackBuffer<'_>` →
//! `BorrowedForDma<'_, _>` → `MockBlitter::start_fill_typed` →
//! `InFlight<'_, _>` → released back to the caller. The mock records
//! the submission's typed handle parameters; the test asserts they
//! match what `Dma2dBlitter::start_fill_typed` would have written into
//! the DMA2D OMAR/OOR/OPFCCR/NLR registers on real hardware.
//!
//! Runs as part of `cargo test -p rlvgl-example-disco-sim` (Phase 4.5
//! of the pre-publish gate). Activates the `mock_blitter` feature on
//! `rlvgl-platform` via the dev-dependency declared in `Cargo.toml`.

use rlvgl_platform::BlitRect;
use rlvgl_platform::PixelFmt;
use rlvgl_platform::hwcore::addr::{PhysAddr, SDRAM_BANK_STRIDE, SDRAM_BANK2_BASE};
use rlvgl_platform::hwcore::mock::{MockBlitter, MockOp};
use rlvgl_platform::hwcore::surface::{BackBuffer, FrameBuffer, Scanout};

const W: u32 = 480;
const H: u32 = 272;
const STRIDE: u32 = W * 4; // ARGB8888

fn fb_at(addr: u32) -> FrameBuffer {
    // SAFETY: the mock never dereferences the address. Real hardware
    // submissions would dereference; the typed contract makes the
    // unsafety explicit at construction.
    unsafe { FrameBuffer::from_phys(PhysAddr::new(addr), W, H, STRIDE, PixelFmt::Argb8888) }
}

#[test]
fn fill_typed_records_dma_address_stride_and_geometry() {
    let mut fb = fb_at(SDRAM_BANK2_BASE);
    let mut back = BackBuffer::wrap(&mut fb);
    let mut blitter = MockBlitter::new();

    let dst = back.dma_dst();
    let inflight = blitter.start_fill_typed(
        dst,
        BlitRect {
            x: 10,
            y: 20,
            w: 100,
            h: 100,
        },
        0xFF11_2233,
    );
    let _released = inflight.into_borrow();

    let op = blitter.last().expect("one submission recorded");
    match op {
        MockOp::Fill {
            dst_addr,
            dst_stride,
            format,
            area,
            color,
        } => {
            // Rect origin (x=10, y=20) applied: addr = base + y*stride + x*bpp.
            assert_eq!(
                *dst_addr,
                SDRAM_BANK2_BASE + 20 * STRIDE + 10 * 4,
                "OMAR write target should target the rect origin, not the buffer base"
            );
            assert_eq!(*dst_stride, STRIDE, "stride passed through unchanged");
            assert_eq!(*format, PixelFmt::Argb8888);
            assert_eq!(area.x, 10);
            assert_eq!(area.y, 20);
            assert_eq!(area.w, 100);
            assert_eq!(area.h, 100);
            assert_eq!(*color, 0xFF11_2233);
        }
        other => panic!("expected Fill, got {other:?}"),
    }
}

#[test]
fn scanout_swap_alternates_dma_destination_address() {
    let front = fb_at(SDRAM_BANK2_BASE);
    let back = fb_at(SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE);
    let mut sc = Scanout::try_new(front, back).expect("disjoint banks");
    let mut blitter = MockBlitter::new();

    // Fill the back buffer.
    {
        let mut back_handle = sc.back_mut();
        let dst = back_handle.dma_dst();
        let _ = blitter
            .start_fill_typed(
                dst,
                BlitRect {
                    x: 0,
                    y: 0,
                    w: W,
                    h: H,
                },
                0x0000_00FF,
            )
            .into_borrow();
    }

    // Promote it to front; now the *new* back is the previous front.
    sc.swap();

    {
        let mut back_handle = sc.back_mut();
        let dst = back_handle.dma_dst();
        let _ = blitter
            .start_fill_typed(
                dst,
                BlitRect {
                    x: 0,
                    y: 0,
                    w: W,
                    h: H,
                },
                0x00FF_0000,
            )
            .into_borrow();
    }

    let history = blitter.history();
    assert_eq!(history.len(), 2);
    let MockOp::Fill {
        dst_addr: first, ..
    } = history[0]
    else {
        panic!("expected Fill")
    };
    let MockOp::Fill {
        dst_addr: second, ..
    } = history[1]
    else {
        panic!("expected Fill")
    };
    assert_ne!(
        first, second,
        "swap() must point DMA at the other physical bank"
    );
    assert_eq!(first, SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE);
    assert_eq!(second, SDRAM_BANK2_BASE);
}

#[test]
fn inflight_lifecycle_returns_borrow_for_reuse() {
    let mut fb = fb_at(SDRAM_BANK2_BASE);
    let mut back = BackBuffer::wrap(&mut fb);
    let mut blitter = MockBlitter::new();

    // First submission.
    let dst = back.dma_dst();
    let inflight = blitter.start_fill_typed(
        dst,
        BlitRect {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        },
        0xFF00_0000,
    );
    let released = inflight.into_borrow();
    drop(released);

    // Second submission requires the BackBuffer borrow back; this only
    // type-checks because the InFlight from the first submission is
    // gone and the implicit reborrow has been released.
    let dst2 = back.dma_dst();
    let _ = blitter
        .start_fill_typed(
            dst2,
            BlitRect {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            },
            0x00FF_0000,
        )
        .into_borrow();

    assert_eq!(blitter.history().len(), 2);
}
