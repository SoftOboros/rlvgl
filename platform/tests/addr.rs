//! Integration tests for `hwcore::addr` newtypes.
//!
//! These tests exercise the public surface from an external crate
//! perspective and verify the SDRAM bank-collision math that previously
//! lived inline at `examples/stm32h747i-disco/src/main.rs:2890-2891`.

use rlvgl_platform::hwcore::addr::{SDRAM_BANK_COUNT, SDRAM_BANK_STRIDE, SDRAM_BANK2_BASE};
use rlvgl_platform::{AddrError, DmaAddr, PhysAddr};

#[test]
fn sdram_bank_partitions_match_example_constants() {
    // The framebuffer placement strategy in the 747I example assumes:
    //   front = SDRAM_BANK2_BASE
    //   back  = SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE
    // must resolve to distinct bank indices.
    let front = PhysAddr::new(SDRAM_BANK2_BASE);
    let back = PhysAddr::new(SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE);
    let fb = front.sdram_bank().expect("front in-range");
    let bb = back.sdram_bank().expect("back in-range");
    assert_ne!(
        fb, bb,
        "front/back framebuffers must live in distinct banks"
    );
    assert_eq!(fb, 0);
    assert_eq!(bb, 1);
}

#[test]
fn sdram_bank_rejects_out_of_range_addresses() {
    assert_eq!(PhysAddr::new(0x2000_0000).sdram_bank(), None);
    let too_high = SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE * u32::from(SDRAM_BANK_COUNT);
    assert_eq!(PhysAddr::new(too_high).sdram_bank(), None);
}

#[test]
fn dma_addr_from_phys_enforces_argb8888_alignment() {
    // ARGB8888 DMA2D transfers require 4-byte alignment on OMAR.
    let misaligned = PhysAddr::new(SDRAM_BANK2_BASE + 1);
    assert!(matches!(
        DmaAddr::from_phys(misaligned, 4),
        Err(AddrError::Misaligned { required: 4, .. })
    ));

    let aligned = PhysAddr::new(SDRAM_BANK2_BASE + 8);
    assert_eq!(
        DmaAddr::from_phys(aligned, 4).unwrap().raw(),
        SDRAM_BANK2_BASE + 8
    );
}

#[test]
fn dma_addr_rejects_zero_alignment() {
    let p = PhysAddr::new(SDRAM_BANK2_BASE);
    assert!(matches!(
        DmaAddr::from_phys(p, 0),
        Err(AddrError::InvalidAlignment(0))
    ));
}
