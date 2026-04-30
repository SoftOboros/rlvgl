//! DPI panel controller — 800×480 RGB888, Pi 7" video timings.
//!
//! Equivalent of:
//! ```c
//! esp_lcd_dpi_panel_config_t dpi_cfg = { /* ... 26 MHz, RGB888, ... */ };
//! esp_lcd_new_panel_dpi(dsi_bus, &dpi_cfg, &dpi_panel);
//! esp_lcd_panel_init(dpi_panel);
//! ```
//!
//! Two entry points:
//!
//! 1. [`DpiPanel::init_pattern`] — first-light variant. Uses the DSI
//!    Host's built-in video pattern generator (`vid_mode_cfg.vpg_en`)
//!    to send a vertical-bar test pattern down the DSI link. No
//!    framebuffer, no DMA, no PSRAM. The bridge's DPI output is
//!    disabled while the pattern generator runs (the IDF
//!    `esp_lcd_dpi_panel_set_pattern` does the same dance).
//!
//! 2. [`DpiPanel::init_with_fb`] — full DMA path with a PSRAM-backed
//!    framebuffer. Requires DW-GDMA bring-up (~hundreds more register
//!    writes) which is staged separately. Currently returns
//!    `Err(DpiError::Unimplemented)` — the bounded host + bridge
//!    register surface is set up so only the DMA channel allocation
//!    and link list need to be added once 5b.6 lands.
//!
//! Derived from `components/esp_lcd/dsi/esp_lcd_panel_dpi.c` plus the LL
//! headers `mipi_dsi_host_ll.h` and `mipi_dsi_brg_ll.h` in
//! `components/hal/esp32p4/include/hal/`.

#![allow(dead_code)]

use super::dsi_host::DsiBus;
use esp32p4 as pac;

/// Built-in DSI Host pattern generator modes.
#[derive(Copy, Clone, Debug)]
pub enum PatternType {
    /// Vertical color bars (vpg_mode=0, orientation=0).
    BarVertical,
    /// Horizontal color bars (vpg_mode=0, orientation=1).
    BarHorizontal,
    /// Vertical bit-error-rate pattern (vpg_mode=1).
    BerVertical,
}

/// 800×480 RGB888 framebuffer view (returned by [`DpiPanel::init_with_fb`]).
pub struct FrameBuffer<'p> {
    pub ptr: *mut u8,
    pub len: usize,
    _phantom: core::marker::PhantomData<&'p mut ()>,
}

/// Opaque DPI panel handle.
pub struct DpiPanel {
    _private: (),
}

#[derive(Debug)]
pub enum DpiError {
    /// DMA-from-FB path not yet ported (DW-GDMA bring-up pending).
    Unimplemented,
    InvalidArg,
}

/// Total framebuffer size in bytes (RGB888): 800 × 480 × 3 = 1_152_000.
pub const FB_BYTES: usize = super::H_RES as usize * super::V_RES as usize * 3;

const RGB888_COLOR_CODE: u8 = 5;
const BRIDGE_RAW_TYPE_RGB888: u8 = 0;
const VIDEO_BURST_WITH_SYNC_PULSES: u8 = 2;
const BPP_RGB888: u32 = 24;

impl DpiPanel {
    /// First-light variant: drive the panel from the DSI Host's internal
    /// pattern generator. Configures all DPI host + bridge timing
    /// registers, but bypasses the bridge data path (no DMA).
    ///
    /// # Safety
    /// `dsi` must have been initialized by [`super::dsi_host::init`].
    /// The DPI clock must already be configured via
    /// [`super::dsi_host::clocks::enable_dpi_clock`].
    pub unsafe fn init_pattern(
        dsi: &DsiBus,
        dpi_clock_freq_mhz: u32,
        pattern: PatternType,
    ) -> Result<Self, DpiError> {
        if dpi_clock_freq_mhz == 0 {
            return Err(DpiError::InvalidArg);
        }

        let p = unsafe { pac::Peripherals::steal() };
        let host = &p.MIPI_DSI_HOST;
        let bridge = &p.MIPI_DSI_BRIDGE;

        // dpi2lane_clk_ratio = lane_bit_rate / dpi_clk / 8 (in lane byte
        // clocks per DPI pixel). Used to scale H timing from pixels to
        // lane byte clock cycles.
        let lane_byte_per_pixel = dsi.lane_bit_rate_mbps / 8 / dpi_clock_freq_mhz;

        let h_active: u32 = super::H_RES as u32;
        let h_sa: u32 = super::H_SYNC as u32;
        let h_bp: u32 = super::H_BACK_PORCH as u32;
        let h_fp: u32 = super::H_FRONT_PORCH as u32;
        let v_active: u32 = super::V_RES as u32;
        let v_sa: u32 = super::V_SYNC as u32;
        let v_bp: u32 = super::V_BACK_PORCH as u32;
        let v_fp: u32 = super::V_FRONT_PORCH as u32;
        // Host vertical-timing fields are 16-bit (12 bits used).
        let v_active_u16 = v_active as u16;
        let v_sa_u16 = v_sa as u16;
        let v_bp_u16 = v_bp as u16;
        let v_fp_u16 = v_fp as u16;

        // ----- DSI Host DPI register block --------------------------------
        // dpi_vcid = 0
        host.dpi_vcid()
            .modify(|_, w| unsafe { w.dpi_vcid().bits(0) });
        // dpi_color_coding = 5 (RGB888)
        host.dpi_color_coding()
            .modify(|_, w| unsafe { w.dpi_color_coding().bits(RGB888_COLOR_CODE) });
        // dpi_cfg_pol: all signals active-high
        host.dpi_cfg_pol().modify(|_, w| {
            w.hsync_active_low().clear_bit();
            w.vsync_active_low().clear_bit();
            w.dataen_active_low().clear_bit();
            w.shutd_active_low().clear_bit();
            w.colorm_active_low().clear_bit();
            w
        });

        // vid_mode_cfg: lp_*_en=1 (allow LP transitions in porches),
        // frame_bta_ack_en=1, lp_cmd_en=1, vid_mode_type = burst with
        // sync pulses. vpg_en gets set later if pattern != None.
        host.vid_mode_cfg().modify(|_, w| {
            unsafe { w.vid_mode_type().bits(VIDEO_BURST_WITH_SYNC_PULSES) };
            w.lp_vsa_en().set_bit();
            w.lp_vbp_en().set_bit();
            w.lp_vfp_en().set_bit();
            w.lp_vact_en().set_bit();
            w.lp_hbp_en().set_bit();
            w.lp_hfp_en().set_bit();
            w.frame_bta_ack_en().set_bit();
            w.lp_cmd_en().set_bit();
            w
        });

        // vid_pkt_size = h_active (1 packet per line, no chunks/null).
        // vid_pkt_size is 14 bits (u16), num_chunks/null_size are 13/13 bits (u16).
        host.vid_pkt_size()
            .modify(|_, w| unsafe { w.vid_pkt_size().bits(h_active as u16) });
        host.vid_num_chunks()
            .modify(|_, w| unsafe { w.vid_num_chunks().bits(0) });
        host.vid_null_size()
            .modify(|_, w| unsafe { w.vid_null_size().bits(0) });

        // Horizontal timing in lane byte clock cycles. hsa/hbp are
        // 12-bit u16; hline_time is 15-bit u16.
        let host_hsa = (h_sa * lane_byte_per_pixel) as u16;
        let host_hbp = (h_bp * lane_byte_per_pixel) as u16;
        let host_h_act = h_active * lane_byte_per_pixel;
        let host_hfp = h_fp * lane_byte_per_pixel;
        let host_hline = (host_h_act + (host_hsa as u32) + (host_hbp as u32) + host_hfp) as u16;
        host.vid_hsa_time()
            .modify(|_, w| unsafe { w.vid_hsa_time().bits(host_hsa) });
        host.vid_hbp_time()
            .modify(|_, w| unsafe { w.vid_hbp_time().bits(host_hbp) });
        host.vid_hline_time()
            .modify(|_, w| unsafe { w.vid_hline_time().bits(host_hline) });

        // Vertical timing in lines.
        host.vid_vsa_lines()
            .modify(|_, w| unsafe { w.vsa_lines().bits(v_sa_u16) });
        host.vid_vbp_lines()
            .modify(|_, w| unsafe { w.vbp_lines().bits(v_bp_u16) });
        host.vid_vactive_lines()
            .modify(|_, w| unsafe { w.v_active_lines().bits(v_active_u16) });
        host.vid_vfp_lines()
            .modify(|_, w| unsafe { w.vfp_lines().bits(v_fp_u16) });

        // ----- DSI Bridge register block ----------------------------------
        // pixel_type: raw_type=0 (RGB888), dpi_config=0, data_in_type=0 (RGB).
        bridge.pixel_type().modify(|_, w| {
            unsafe {
                w.raw_type().bits(BRIDGE_RAW_TYPE_RGB888);
                w.dpi_config().bits(0);
            }
            w.data_in_type().clear_bit();
            w
        });

        // Horizontal timing (pixels). H fields are 12-bit (bits 0:11).
        let h_total = (h_active + h_sa + h_bp + h_fp) as u16;
        bridge.dpi_h_cfg0().modify(|_, w| unsafe {
            w.hdisp().bits(h_active as u16);
            w.htotal().bits(h_total);
            w
        });
        bridge.dpi_h_cfg1().modify(|_, w| unsafe {
            w.hsync().bits(h_sa as u16);
            w.hbank().bits(h_bp as u16);
            w
        });

        // Vertical timing (lines). V fields are 12-bit (bits 0:11).
        let v_total = (v_active + v_sa + v_bp + v_fp) as u16;
        bridge.dpi_v_cfg0().modify(|_, w| unsafe {
            w.vdisp().bits(v_active as u16);
            w.vtotal().bits(v_total);
            w
        });
        bridge.dpi_v_cfg1().modify(|_, w| unsafe {
            w.vsync().bits(v_sa as u16);
            w.vbank().bits(v_bp as u16);
            w
        });

        // num_pixel_bits = h * v * bpp, in 64-bit words. raw_num_total is 22 bits.
        let num_pixel_bits = h_active * v_active * BPP_RGB888;
        bridge.raw_num_cfg().modify(|_, w| unsafe {
            w.raw_num_total().bits(num_pixel_bits / 64);
            w.raw_num_total_set().set_bit();
            w
        });

        // Underrun discard count = h_active. Field is 12 bits (bits 4:15).
        bridge
            .dpi_misc_config()
            .modify(|_, w| unsafe { w.fifo_underrun_discard_vcnt().bits(h_active as u16) });

        // Flow controller = DMA (bit clear; matches IDF default).
        bridge
            .dma_flow_ctrl()
            .modify(|_, w| w.dsi_dma_flow_controller().clear_bit());

        // Enable bridge module so bridge-side state machines are alive
        // even though we'll disable dpi_en during pattern mode.
        bridge.en().write(|w| w.dsi_en().set_bit());
        bridge
            .dpi_config_update()
            .write(|w| w.dpi_config_update().set_bit());

        // ----- Pattern generator + video mode entry -----------------------
        // Stop bridge DPI output while VPG runs (per
        // esp_lcd_dpi_panel_set_pattern):
        bridge
            .dpi_misc_config()
            .modify(|_, w| w.dpi_en().clear_bit());
        bridge
            .dpi_config_update()
            .write(|w| w.dpi_config_update().set_bit());

        // Enable the host VPG with the requested pattern.
        host.vid_mode_cfg().modify(|_, w| {
            match pattern {
                PatternType::BarVertical => {
                    w.vpg_mode().clear_bit();
                    w.vpg_orientation().clear_bit();
                    w.vpg_en().set_bit();
                }
                PatternType::BarHorizontal => {
                    w.vpg_mode().clear_bit();
                    w.vpg_orientation().set_bit();
                    w.vpg_en().set_bit();
                }
                PatternType::BerVertical => {
                    w.vpg_mode().set_bit();
                    w.vpg_orientation().clear_bit();
                    w.vpg_en().set_bit();
                }
            };
            w
        });

        // Switch host into video mode (cmd_video_mode = 0).
        host.mode_cfg()
            .modify(|_, w| w.cmd_video_mode().clear_bit());

        // Switch the clock lane to AUTO (HS during video, LP between).
        host.lpclk_ctrl().modify(|_, w| {
            w.auto_clklane_ctrl().set_bit();
            w.phy_txrequestclkhs().set_bit();
            w
        });

        Ok(Self { _private: () })
    }

    /// Full DMA path: scan a PSRAM-backed framebuffer to the panel.
    ///
    /// Stub. Requires DW-GDMA bring-up (esp32p4 PAC `dma`/`dw_gdma`
    /// peripheral) which is the next phase. The host + bridge config
    /// performed here is identical to [`init_pattern`]; the missing
    /// piece is the DMA channel allocation, link-list setup, and
    /// `mipi_dsi_brg_ll_enable_dpi_output(true)` instead of the
    /// pattern-generator path.
    ///
    /// # Safety
    /// As `init_pattern`, plus PSRAM mapped + cache-coherent.
    pub unsafe fn init_with_fb(_dsi: &DsiBus) -> Result<(Self, FrameBuffer<'static>), DpiError> {
        Err(DpiError::Unimplemented)
    }
}
