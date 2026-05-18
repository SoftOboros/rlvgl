//! Typed framebuffer ownership.
//!
//! Framebuffer state flows as typed handles — not as `u32` addresses — so
//! the borrow checker enforces the display-pipeline contract at compile
//! time:
//!
//! - [`FrontBuffer<'a>`] is the LTDC scan-out view. CPU writes are impossible
//!   because the type only exposes `&` methods — `cpu_slice_mut` does not
//!   exist.
//! - [`BackBuffer<'a>`] is the renderer/DMA2D target. CPU writes are allowed
//!   via [`BackBuffer::cpu_slice`] (unsafe). Submitting the buffer to a DMA
//!   engine consumes a [`BorrowedForDma`] reborrow; while that token is
//!   alive the `&mut BackBuffer` is held, so `cpu_slice` is rejected by
//!   rustc as a double-borrow.
//! - [`InFlight<'dma, T>`] owns a `BorrowedForDma<'dma, T>` for the duration
//!   of a DMA submission. Dropping / completing the `InFlight` returns the
//!   buffer's `&mut` borrow to the caller.
//! - [`Scanout`] owns both [`FrameBuffer`]s and hands out a `FrontBuffer<'_>`
//!   view plus a `BackBuffer<'_>` mutably-borrowed handle. `back_mut` cannot
//!   be called twice simultaneously — rustc rejects the second call as
//!   E0499.
//!
//! This module is **additive** in Step 2: no callers migrate yet. The
//! typed DMA2D API that consumes `BorrowedForDma<'_, BackBuffer<'_>>` lands
//! in Step 3 (`platform::dma2d::Dma2dBlitter::start_fill_typed`).

use core::marker::PhantomData;

use crate::blit::PixelFmt;
use crate::hwcore::addr::{DmaAddr, PhysAddr};

// ─── FrameBuffer ────────────────────────────────────────────────────────

/// An owned physical framebuffer region.
///
/// Created once per SDRAM allocation at boot. Owns geometry metadata plus
/// the [`PhysAddr`] — no raw pointer, no `u32` escape hatch in the public
/// API.
#[derive(Debug)]
pub struct FrameBuffer {
    addr: PhysAddr,
    width: u32,
    height: u32,
    stride_bytes: u32,
    format: PixelFmt,
    bank: u8,
}

/// Sentinel used by [`FrameBuffer::bank`] for addresses outside the SDRAM
/// Bank 2 window (e.g. SRAM-backed mock buffers during tests).
const BANK_OUT_OF_RANGE: u8 = u8::MAX;

impl FrameBuffer {
    /// Construct a [`FrameBuffer`] at a physical address.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    ///
    /// - `[addr, addr + stride_bytes * height)` is a mapped, writable RAM
    ///   region for the `'static` lifetime of the returned value,
    /// - no other `FrameBuffer`, `&mut [u8]`, or DMA master references
    ///   overlapping bytes,
    /// - `addr` is appropriately aligned for `format` (4 bytes for
    ///   `Argb8888`, 2 bytes for `Rgb565`, 1 byte for `L8`/`A8`/`A4`),
    /// - `stride_bytes >= width * pixel_size(format)`.
    pub unsafe fn from_phys(
        addr: PhysAddr,
        width: u32,
        height: u32,
        stride_bytes: u32,
        format: PixelFmt,
    ) -> Self {
        let bank = addr.sdram_bank().unwrap_or(BANK_OUT_OF_RANGE);
        Self {
            addr,
            width,
            height,
            stride_bytes,
            format,
            bank,
        }
    }

    /// Physical base address.
    #[inline]
    pub fn addr(&self) -> PhysAddr {
        self.addr
    }

    /// DMA bus address for handoff to a DMA master (DMA2D OMAR, LTDC
    /// CFBAR, etc.).
    ///
    /// The conversion re-asserts pixel-format alignment; a misaligned
    /// framebuffer address is a construction-time bug but `dma_addr`
    /// falls back to byte alignment so callers receive a usable value
    /// rather than a panic. Wire errors are surfaced via the pixel
    /// validator in the DMA engine itself.
    pub fn dma_addr(&self) -> DmaAddr {
        let align = pixel_size(self.format);
        DmaAddr::from_phys(self.addr, align)
            .or_else(|_| DmaAddr::from_phys(self.addr, 1))
            .expect("byte-aligned DmaAddr always valid for a non-zero alignment")
    }

    /// Width in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Row stride in bytes.
    #[inline]
    pub fn stride_bytes(&self) -> u32 {
        self.stride_bytes
    }

    /// Pixel format.
    #[inline]
    pub fn format(&self) -> PixelFmt {
        self.format
    }

    /// SDRAM bank index (0..`SDRAM_BANK_COUNT`), or `None` if this
    /// framebuffer lives outside the Bank 2 window.
    #[inline]
    pub fn bank(&self) -> Option<u8> {
        if self.bank == BANK_OUT_OF_RANGE {
            None
        } else {
            Some(self.bank)
        }
    }

    /// Total framebuffer size in bytes (`stride_bytes * height`).
    #[inline]
    pub fn byte_len(&self) -> usize {
        self.stride_bytes as usize * self.height as usize
    }
}

/// Bytes-per-pixel for a given [`PixelFmt`], used for alignment checks.
#[inline]
pub const fn pixel_size(fmt: PixelFmt) -> usize {
    match fmt {
        PixelFmt::Argb8888 => 4,
        PixelFmt::Rgb565 => 2,
        PixelFmt::L8 | PixelFmt::A8 | PixelFmt::A4 => 1,
    }
}

// ─── FrontBuffer ────────────────────────────────────────────────────────

/// Shared read-only view of a framebuffer currently handed to the display
/// scan-out engine (LTDC CFBAR).
///
/// Deliberately exposes no `&mut` accessors: the scan-out path reads these
/// pixels at pixel-clock rate, so CPU writes would tear. Obtaining write
/// access requires swapping this buffer out via [`Scanout::swap`] first.
pub struct FrontBuffer<'a> {
    fb: &'a FrameBuffer,
}

impl<'a> FrontBuffer<'a> {
    /// Wrap a shared reference to a [`FrameBuffer`].
    #[inline]
    pub fn wrap(fb: &'a FrameBuffer) -> Self {
        Self { fb }
    }

    /// Underlying [`FrameBuffer`].
    #[inline]
    pub fn framebuffer(&self) -> &FrameBuffer {
        self.fb
    }

    /// Physical base address.
    #[inline]
    pub fn addr(&self) -> PhysAddr {
        self.fb.addr()
    }

    /// DMA bus address (for writing into LTDC CFBAR).
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.fb.dma_addr()
    }

    /// Width in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.fb.width()
    }

    /// Height in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.fb.height()
    }

    /// Row stride in bytes.
    #[inline]
    pub fn stride_bytes(&self) -> u32 {
        self.fb.stride_bytes()
    }

    /// Pixel format.
    #[inline]
    pub fn format(&self) -> PixelFmt {
        self.fb.format()
    }
}

// ─── BackBuffer ─────────────────────────────────────────────────────────

/// Mutable view of a framebuffer the renderer currently owns.
///
/// CPU writes happen through [`BackBuffer::cpu_slice`]. DMA submissions go
/// through [`BackBuffer::dma_dst`], which reborrows the buffer and locks
/// out CPU access for the duration of the transfer.
pub struct BackBuffer<'a> {
    fb: &'a mut FrameBuffer,
}

impl<'a> BackBuffer<'a> {
    /// Wrap a mutable reference to a [`FrameBuffer`].
    #[inline]
    pub fn wrap(fb: &'a mut FrameBuffer) -> Self {
        Self { fb }
    }

    /// Underlying [`FrameBuffer`].
    #[inline]
    pub fn framebuffer(&self) -> &FrameBuffer {
        self.fb
    }

    /// Underlying [`FrameBuffer`] (mutable metadata, not pixels).
    #[inline]
    pub fn framebuffer_mut(&mut self) -> &mut FrameBuffer {
        self.fb
    }

    /// Physical base address.
    #[inline]
    pub fn addr(&self) -> PhysAddr {
        self.fb.addr()
    }

    /// DMA bus address (for writing into DMA2D OMAR).
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.fb.dma_addr()
    }

    /// Width in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.fb.width()
    }

    /// Height in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.fb.height()
    }

    /// Row stride in bytes.
    #[inline]
    pub fn stride_bytes(&self) -> u32 {
        self.fb.stride_bytes()
    }

    /// Pixel format.
    #[inline]
    pub fn format(&self) -> PixelFmt {
        self.fb.format()
    }

    /// Produce a reborrow suitable for DMA-engine submission.
    ///
    /// The returned [`BorrowedForDma`] carries a mutable reborrow of this
    /// back buffer for lifetime `'b`. While it — or any [`InFlight`]
    /// derived from it — is alive, `self` is borrowed and CPU access via
    /// [`BackBuffer::cpu_slice`] is rejected by the borrow checker.
    #[inline]
    pub fn dma_dst<'b>(&'b mut self) -> BorrowedForDma<'b, BackBuffer<'a>> {
        BorrowedForDma {
            inner: self,
            _phantom: PhantomData,
        }
    }

    /// CPU byte access to the back buffer's RAM.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    ///
    /// - no DMA master is currently reading or writing the buffer (the
    ///   [`InFlight`] lifecycle already encodes this for DMA2D paths),
    /// - the RAM region is actually mapped and writable (established by
    ///   [`FrameBuffer::from_phys`]),
    /// - no other `&mut [u8]` or `&[u8]` referencing overlapping bytes is
    ///   alive for the returned slice's lifetime.
    pub unsafe fn cpu_slice(&mut self) -> &mut [u8] {
        let len = self.fb.byte_len();
        // SAFETY: delegated to caller per the contract above; `len` is
        // computed from the framebuffer geometry owned by this handle.
        unsafe { self.fb.addr.as_mut_slice(len) }
    }
}

// ─── BorrowedForDma + InFlight ─────────────────────────────────────────

/// A framebuffer reborrow tagged as "handed to a DMA master for the
/// duration of this value's lifetime".
///
/// Holds an `&'a mut T`, so while alive it prevents both a second DMA
/// submission and any CPU access via the source handle. Produced by
/// [`BackBuffer::dma_dst`]; consumed by DMA-engine submission APIs that
/// return an [`InFlight`].
pub struct BorrowedForDma<'a, T: ?Sized + 'a> {
    inner: &'a mut T,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, 'fb> BorrowedForDma<'a, BackBuffer<'fb>> {
    /// DMA bus address of the borrowed back buffer.
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.inner.dma_addr()
    }

    /// Width in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.inner.width()
    }

    /// Height in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.inner.height()
    }

    /// Row stride in bytes.
    #[inline]
    pub fn stride_bytes(&self) -> u32 {
        self.inner.stride_bytes()
    }

    /// Pixel format.
    #[inline]
    pub fn format(&self) -> PixelFmt {
        self.inner.format()
    }
}

/// Non-blocking DMA transfer token.
///
/// Owns a [`BorrowedForDma`] for the transfer duration; the `&mut`
/// reborrow inside prevents CPU access and re-submission until the
/// transfer completes. Completion (polling or waiting) consumes the
/// `InFlight` and returns the [`BorrowedForDma`] (which can then be
/// dropped to release the `&mut` back to the caller).
///
/// The concrete `start_*_typed` methods that produce `InFlight` values
/// land with the typed DMA2D API in Step 3 of the refactor plan.
pub struct InFlight<'dma, T: ?Sized + 'dma> {
    borrow: BorrowedForDma<'dma, T>,
}

impl<'dma, T: ?Sized + 'dma> InFlight<'dma, T> {
    /// Wrap a [`BorrowedForDma`] in an `InFlight` — used by DMA engines
    /// to hand the borrow back to the caller on submission.
    #[inline]
    pub fn new(borrow: BorrowedForDma<'dma, T>) -> Self {
        Self { borrow }
    }

    /// Release the underlying [`BorrowedForDma`] — called by DMA-engine
    /// completion handlers when the transfer is done and the CPU may
    /// touch the buffer again.
    #[inline]
    pub fn into_borrow(self) -> BorrowedForDma<'dma, T> {
        self.borrow
    }
}

// ─── Scanout ────────────────────────────────────────────────────────────

/// A pair of framebuffers used as a swap chain by LTDC.
///
/// Holds two [`FrameBuffer`]s and hands out `FrontBuffer<'_>` /
/// `BackBuffer<'_>` views. `back_mut` cannot be called twice
/// simultaneously — a second call fails rustc borrow check (E0499),
/// which is the compile-time proof that the renderer never competes
/// with itself for a single back buffer.
#[derive(Debug)]
pub struct Scanout {
    front: FrameBuffer,
    back: FrameBuffer,
}

/// Error returned by [`Scanout::try_new`] when `front` and `back` would
/// land in the same SDRAM bank — a configuration that causes refresh /
/// scan-line contention on the STM32H747I-DISCO module.
#[derive(Debug)]
pub struct BankCollision {
    /// The front framebuffer the caller supplied.
    pub front: FrameBuffer,
    /// The back framebuffer the caller supplied.
    pub back: FrameBuffer,
    /// The common bank index both buffers resolved to.
    pub bank: u8,
}

impl Scanout {
    /// Build a [`Scanout`] from two framebuffers.
    ///
    /// Returns [`BankCollision`] if both framebuffers resolve to the same
    /// SDRAM bank. Framebuffers whose `bank()` returns `None` (non-SDRAM
    /// RAM, test mocks) are accepted without a collision check.
    pub fn try_new(front: FrameBuffer, back: FrameBuffer) -> Result<Self, BankCollision> {
        if let (Some(fb), Some(bb)) = (front.bank(), back.bank())
            && fb == bb
        {
            return Err(BankCollision {
                front,
                back,
                bank: fb,
            });
        }
        Ok(Self { front, back })
    }

    /// Read-only view of the currently-scanning front buffer.
    #[inline]
    pub fn front(&self) -> FrontBuffer<'_> {
        FrontBuffer::wrap(&self.front)
    }

    /// Mutable handle on the currently-rendering back buffer.
    ///
    /// Calling this twice without dropping the first returned value is a
    /// compile error (E0499). This is the mechanism that prevents the
    /// renderer and an overlay path from both holding back-buffer
    /// borrows simultaneously.
    #[inline]
    pub fn back_mut(&mut self) -> BackBuffer<'_> {
        BackBuffer::wrap(&mut self.back)
    }

    /// Swap front and back. Typically called at LTDC vertical-sync after
    /// the back buffer has been fully rendered and any outstanding
    /// [`InFlight`] has been consumed.
    #[inline]
    pub fn swap(&mut self) {
        core::mem::swap(&mut self.front, &mut self.back);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hwcore::addr::{SDRAM_BANK_STRIDE, SDRAM_BANK2_BASE};

    const W: u32 = 480;
    const H: u32 = 272;
    const STRIDE: u32 = W * 4;

    fn mk(addr: u32) -> FrameBuffer {
        // SAFETY: test only; the address is never dereferenced.
        unsafe { FrameBuffer::from_phys(PhysAddr::new(addr), W, H, STRIDE, PixelFmt::Argb8888) }
    }

    #[test]
    fn bank_disjoint_framebuffers_pair_successfully() {
        let front = mk(SDRAM_BANK2_BASE);
        let back = mk(SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE);
        let sc = Scanout::try_new(front, back).expect("disjoint banks");
        assert_eq!(sc.front().width(), W);
    }

    #[test]
    fn same_bank_framebuffers_are_rejected() {
        let front = mk(SDRAM_BANK2_BASE);
        let back = mk(SDRAM_BANK2_BASE + 0x100); // same bank
        let err = Scanout::try_new(front, back).unwrap_err();
        assert_eq!(err.bank, 0);
    }

    #[test]
    fn frontbuffer_reports_geometry() {
        let fb = mk(SDRAM_BANK2_BASE);
        let front = FrontBuffer::wrap(&fb);
        assert_eq!(front.width(), W);
        assert_eq!(front.height(), H);
        assert_eq!(front.stride_bytes(), STRIDE);
        assert_eq!(front.format(), PixelFmt::Argb8888);
    }

    #[test]
    fn backbuffer_dma_dst_reborrow_is_well_formed() {
        let mut fb = mk(SDRAM_BANK2_BASE);
        let mut back = BackBuffer::wrap(&mut fb);
        {
            let dst = back.dma_dst();
            assert_eq!(dst.width(), W);
            assert_eq!(dst.stride_bytes(), STRIDE);
            // `dst` drops here, releasing the reborrow.
        }
        // Now `back` is usable again — this only needs to type-check.
        let _dst2 = back.dma_dst();
    }

    #[test]
    fn scanout_swap_exchanges_buffers() {
        let mut sc = Scanout::try_new(
            mk(SDRAM_BANK2_BASE),
            mk(SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE),
        )
        .unwrap();
        let before = sc.front().addr();
        sc.swap();
        let after = sc.front().addr();
        assert_ne!(before, after);
    }

    #[test]
    fn inflight_round_trips_borrow() {
        let mut fb = mk(SDRAM_BANK2_BASE);
        let mut back = BackBuffer::wrap(&mut fb);
        let dst = back.dma_dst();
        let inflight = InFlight::new(dst);
        let released = inflight.into_borrow();
        assert_eq!(released.width(), W);
    }

    #[test]
    fn pixel_size_matches_format() {
        assert_eq!(pixel_size(PixelFmt::Argb8888), 4);
        assert_eq!(pixel_size(PixelFmt::Rgb565), 2);
        assert_eq!(pixel_size(PixelFmt::L8), 1);
    }
}
