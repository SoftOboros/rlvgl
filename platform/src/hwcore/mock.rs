//! Host-only mock blitter for the typed DMA2D submission API.
//!
//! `MockBlitter` mirrors the shape of [`Dma2dBlitter`'s][`crate::dma2d`]
//! `start_*_typed` methods so host tests can exercise the
//! [`BackBuffer::dma_dst`] / [`InFlight`] borrow lifecycle without an
//! ARM target.
//!
//! Submitted operations are recorded in an in-memory history that the
//! test then queries — the mock does not write into the underlying RAM.
//! The `BorrowedForDma` reborrow it consumes is genuine, so the borrow
//! checker enforces the same "no CPU touch during DMA" contract that
//! production code relies on.
//!
//! Gated by the `mock_blitter` Cargo feature (off by default). In tests
//! within this crate it is also enabled by `#[cfg(test)]`.

extern crate alloc;

use alloc::vec::Vec;

use crate::blit::{PixelFmt, Rect};
use crate::hwcore::surface::{BackBuffer, BorrowedForDma, InFlight, pixel_size};

/// One recorded operation submitted to a [`MockBlitter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockOp {
    /// `start_fill_typed` recording — the DMA bus address is recorded
    /// rather than the actual pixel write so tests stay independent of
    /// the underlying memory layout.
    Fill {
        /// DMA bus address pulled from the submitted [`BorrowedForDma`].
        dst_addr: u32,
        /// Stride in bytes pulled from the submitted [`BorrowedForDma`].
        dst_stride: u32,
        /// Pixel format pulled from the submitted [`BorrowedForDma`].
        format: PixelFmt,
        /// The fill rectangle, in back-buffer pixels.
        area: Rect,
        /// Fill colour (ARGB8888).
        color: u32,
    },
    /// `start_blit_typed` recording.
    Blit {
        /// Source pointer (raw, until the typed source-side lands).
        src: usize,
        /// Source stride in bytes.
        src_stride: u32,
        /// Destination DMA bus address from the submitted handle.
        dst_addr: u32,
        /// Destination stride in bytes from the submitted handle.
        dst_stride: u32,
        /// Destination position in back-buffer pixels.
        dst_pos: (i32, i32),
        /// Blit width in pixels.
        width: u32,
        /// Blit height in pixels.
        height: u32,
        /// Pixel format from the submitted handle.
        format: PixelFmt,
    },
}

/// Recording host-side stub for `Dma2dBlitter`'s typed submission API.
#[derive(Debug, Default)]
pub struct MockBlitter {
    history: Vec<MockOp>,
}

impl MockBlitter {
    /// Construct an empty mock blitter.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    /// Mock counterpart of [`Dma2dBlitter::start_fill_typed`][typed].
    ///
    /// Records the submission and returns an [`InFlight`] holding the
    /// `BorrowedForDma` reborrow. The mock completes synchronously — the
    /// caller releases the borrow by consuming the [`InFlight`] via
    /// [`InFlight::into_borrow`] and dropping the result.
    ///
    /// [typed]: ../../dma2d/struct.Dma2dBlitter.html#method.start_fill_typed
    pub fn start_fill_typed<'b, 'fb>(
        &'b mut self,
        dst: BorrowedForDma<'b, BackBuffer<'fb>>,
        area: Rect,
        color: u32,
    ) -> InFlight<'b, BackBuffer<'fb>> {
        let stride = dst.stride_bytes();
        let bpp = pixel_size(dst.format()) as u32;
        let base = dst.dma_addr().raw();
        // Match `Dma2dBlitter::start_fill_typed`: record the computed
        // sub-rectangle origin, not the back-buffer base.
        let dst_addr = base + (area.y as u32) * stride + (area.x as u32) * bpp;
        self.history.push(MockOp::Fill {
            dst_addr,
            dst_stride: stride,
            format: dst.format(),
            area,
            color,
        });
        InFlight::new(dst)
    }

    /// Mock counterpart of [`Dma2dBlitter::start_blit_typed`][typed].
    ///
    /// [typed]: ../../dma2d/struct.Dma2dBlitter.html#method.start_blit_typed
    pub fn start_blit_typed<'b, 'fb>(
        &'b mut self,
        src: *const u8,
        src_stride: u32,
        dst: BorrowedForDma<'b, BackBuffer<'fb>>,
        dst_pos: (i32, i32),
        width: u32,
        height: u32,
    ) -> InFlight<'b, BackBuffer<'fb>> {
        self.history.push(MockOp::Blit {
            src: src as usize,
            src_stride,
            dst_addr: dst.dma_addr().raw(),
            dst_stride: dst.stride_bytes(),
            dst_pos,
            width,
            height,
            format: dst.format(),
        });
        InFlight::new(dst)
    }

    /// Recorded operations, in submission order.
    pub fn history(&self) -> &[MockOp] {
        &self.history
    }

    /// The most-recently submitted operation, if any.
    pub fn last(&self) -> Option<&MockOp> {
        self.history.last()
    }

    /// Reset the recorded history.
    pub fn clear(&mut self) {
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hwcore::addr::{PhysAddr, SDRAM_BANK2_BASE};
    use crate::hwcore::surface::{BackBuffer, FrameBuffer};

    fn fb() -> FrameBuffer {
        // SAFETY: address never dereferenced; mock does not touch memory.
        unsafe {
            FrameBuffer::from_phys(
                PhysAddr::new(SDRAM_BANK2_BASE),
                480,
                272,
                480 * 4,
                PixelFmt::Argb8888,
            )
        }
    }

    #[test]
    fn fill_records_destination_address_and_geometry() {
        let mut fb = fb();
        let mut back = BackBuffer::wrap(&mut fb);
        let mut blitter = MockBlitter::new();
        let dst = back.dma_dst();
        let inflight = blitter.start_fill_typed(
            dst,
            Rect {
                x: 10,
                y: 20,
                w: 100,
                h: 100,
            },
            0xFF11_2233,
        );
        let _ = inflight.into_borrow();
        let op = blitter.last().unwrap().clone();
        match op {
            MockOp::Fill {
                dst_addr,
                dst_stride,
                area,
                color,
                ..
            } => {
                // Rect origin (x=10, y=20) applied over stride 1920.
                assert_eq!(dst_addr, SDRAM_BANK2_BASE + 20 * 1920 + 10 * 4);
                assert_eq!(dst_stride, 480 * 4);
                assert_eq!(
                    area,
                    Rect {
                        x: 10,
                        y: 20,
                        w: 100,
                        h: 100,
                    }
                );
                assert_eq!(color, 0xFF11_2233);
            }
            other => panic!("expected Fill, got {other:?}"),
        }
    }

    #[test]
    fn second_fill_appends_to_history() {
        let mut fb = fb();
        let mut blitter = MockBlitter::new();
        for color in [0xFF11_0000, 0xFF00_FF00, 0xFF00_00FF] {
            let mut back = BackBuffer::wrap(&mut fb);
            let dst = back.dma_dst();
            let _ = blitter
                .start_fill_typed(
                    dst,
                    Rect {
                        x: 0,
                        y: 0,
                        w: 1,
                        h: 1,
                    },
                    color,
                )
                .into_borrow();
        }
        assert_eq!(blitter.history().len(), 3);
    }

    #[test]
    fn clear_drops_history() {
        let mut fb = fb();
        let mut blitter = MockBlitter::new();
        let mut back = BackBuffer::wrap(&mut fb);
        let dst = back.dma_dst();
        let _ = blitter
            .start_fill_typed(
                dst,
                Rect {
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                },
                0,
            )
            .into_borrow();
        blitter.clear();
        assert!(blitter.history().is_empty());
    }
}
