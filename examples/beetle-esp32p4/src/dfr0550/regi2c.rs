//! ESP32-P4 REGI2C analog-bus access (raw-PAC port).
//!
//! BEETLE-06. The DSI DPHY PLL will not lock under our raw-PAC boot even
//! though every memory-mapped register — LDO_VO3 (`PMU_EXT_LDO_VO3`), the
//! 20 MHz PHY reference (`REF_CLK_CTRL1/2`), the PLL M/N test pokes — is
//! confirmed *byte-identical* to a known-good IDF binary by reading both
//! over USB-JTAG (IDF `phy_status.phy_lock = 1`, ours `= 0`). The gap is
//! app-stage system init IDF's `esp_system` startup performs that our bare
//! `esp-riscv-rt` boot skips. See `docs/beetle-esp32p4/ERRATA.md`
//! ERRATA-009.
//!
//! This module ports the chip's internal REGI2C analog bus
//! (faithful to `components/esp_rom/patches/esp_rom_regi2c_esp32p4.c`)
//! and IDF `rtc_clk_init`'s `I2C_BIAS` `FORCE_XPD_*` clears — the analog
//! bias generator. NOTE: [`dphy_analog_bias_init`] was investigated as the
//! DPHY-PLL-lock cause and **eliminated** — a JTAG read showed BIAS
//! register 4 already reads `0x00` (bootloader/default), so the write is a
//! no-op, and the DPHY PLL is not a REGI2C-configured block anyway. The
//! call is kept as faithful IDF system-init parity; the REGI2C primitive
//! is retained as verified infrastructure for any future analog-bus need
//! (e.g. the `I2C_DIG_REG` regulator writes).

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};

// Internal REGI2C analog-bus master (LP_I2C_ANA_MST @ 0x5012_4000) plus the
// LP peripheral clock-enable. Addresses from the esp32p4 TRM / soc headers
// (`i2c_ana_mst_reg.h`, `lpperi_reg.h`).
const I2C_ANA_MST_I2C0_CTRL: *mut u32 = 0x5012_4000 as *mut u32;
const I2C_ANA_MST_ANA_CONF1: *mut u32 = 0x5012_401C as *mut u32;
const I2C_ANA_MST_ANA_CONF2: *mut u32 = 0x5012_4020 as *mut u32;
const I2C_ANA_MST_CLK160M: *mut u32 = 0x5012_4034 as *mut u32;
const LPPERI_CLK_EN: *mut u32 = 0x5012_0000 as *mut u32;

const LPPERI_CK_EN_LP_I2CMST: u32 = 1 << 27;
const CLK_I2C_MST_SEL_160M: u32 = 1 << 0;
const ANA_CONF_FIELD_MASK: u32 = 0x00FF_FFFF;
const BIAS_MST_SEL: u32 = 1 << 12;

// `I2C_ANA_MST_I2C0_CTRL` field layout.
const CTRL_SLAVE_ID_S: u32 = 0; // [7:0]   block / slave id
const CTRL_ADDR_S: u32 = 8; // [15:8]  register address
const CTRL_DATA_S: u32 = 16; // [23:16] data
const CTRL_WR_CNTL: u32 = 1 << 24; // 1 = write, 0 = read
const CTRL_BUSY: u32 = 1 << 25; // transaction in progress

/// REGI2C slave id of the analog BIAS block.
const I2C_BIAS: u32 = 0x6A;

/// Select the BIAS analog block on the REGI2C master and bring the master
/// clock up. Mirrors `regi2c_enable_block(REGI2C_BIAS)`.
///
/// # Safety
/// Direct MMIO to the LP I2C analog master. Run once, early, single-threaded,
/// before any analog calibration touches the same bus.
unsafe fn enable_bias_block() {
    // Enable the LP I2C master peripheral clock.
    unsafe {
        write_volatile(
            LPPERI_CLK_EN,
            read_volatile(LPPERI_CLK_EN) | LPPERI_CK_EN_LP_I2CMST,
        );
        // Drive the master from the 160 MHz clock.
        write_volatile(
            I2C_ANA_MST_CLK160M,
            read_volatile(I2C_ANA_MST_CLK160M) | CLK_I2C_MST_SEL_160M,
        );
        // Clear the device-select fields, then select BIAS on conf2.
        let c1 = read_volatile(I2C_ANA_MST_ANA_CONF1) & !ANA_CONF_FIELD_MASK;
        write_volatile(I2C_ANA_MST_ANA_CONF1, c1);
        let c2 = read_volatile(I2C_ANA_MST_ANA_CONF2) & !ANA_CONF_FIELD_MASK;
        write_volatile(I2C_ANA_MST_ANA_CONF2, c2 | BIAS_MST_SEL);
    }
}

/// # Safety
/// MMIO read of the master control register.
unsafe fn wait_idle() {
    while unsafe { read_volatile(I2C_ANA_MST_I2C0_CTRL) } & CTRL_BUSY != 0 {}
}

/// Read one byte from REGI2C register `reg` of `block`.
///
/// # Safety
/// The target `block` must already be selected via [`enable_bias_block`].
unsafe fn read_reg(block: u32, reg: u32) -> u8 {
    unsafe {
        wait_idle();
        let cmd = (block << CTRL_SLAVE_ID_S) | (reg << CTRL_ADDR_S);
        write_volatile(I2C_ANA_MST_I2C0_CTRL, cmd);
        wait_idle();
        ((read_volatile(I2C_ANA_MST_I2C0_CTRL) >> CTRL_DATA_S) & 0xFF) as u8
    }
}

/// Read-modify-write bits `[msb:lsb]` of REGI2C register `reg` of `block`.
/// Mirrors `regi2c_write_reg_mask_impl`.
///
/// # Safety
/// The target `block` must already be selected via [`enable_bias_block`].
unsafe fn write_reg_mask(block: u32, reg: u32, msb: u32, lsb: u32, data: u8) {
    unsafe {
        let cur = read_reg(block, reg) as u32;
        let width = msb - lsb + 1;
        let field_mask = ((1u32 << width) - 1) << lsb;
        let new = (cur & !field_mask) | (((data as u32) << lsb) & field_mask);
        wait_idle();
        let cmd = (block << CTRL_SLAVE_ID_S)
            | (reg << CTRL_ADDR_S)
            | CTRL_WR_CNTL
            | ((new & 0xFF) << CTRL_DATA_S);
        write_volatile(I2C_ANA_MST_I2C0_CTRL, cmd);
        wait_idle();
    }
}

/// BEETLE-06: clear the analog BIAS generator's `FORCE_XPD_*` fields, for
/// IDF system-init parity.
///
/// Clears the four `I2C_BIAS` `FORCE_XPD_*` fields (CK / REF_OUT_BUF / IPH /
/// VGATE_BUF — BIAS register 4, bits[3:0]) so the bias generator's clock,
/// reference output buffer, current source and vgate buffer are not forced
/// off, matching IDF `rtc_clk_init`.
///
/// NOTE: investigated as the DPHY-PLL-lock cause and **eliminated** — these
/// fields already read `0x00` on this board (bootloader/default), so this
/// is effectively a no-op. Kept for IDF fidelity. See module docs /
/// ERRATA-009.
///
/// # Safety
/// Single-shot early system init; see [`enable_bias_block`].
pub unsafe fn dphy_analog_bias_init() {
    unsafe {
        enable_bias_block();
        // I2C_BIAS reg 4, bits[3:0] = 0:
        //   bit0 FORCE_XPD_CK, bit1 FORCE_XPD_REF_OUT_BUF,
        //   bit2 FORCE_XPD_IPH, bit3 FORCE_XPD_VGATE_BUF.
        write_reg_mask(I2C_BIAS, 4, 3, 0, 0);
    }
}
