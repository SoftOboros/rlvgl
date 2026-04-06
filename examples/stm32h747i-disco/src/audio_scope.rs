//! Audio oscilloscope visualization with phosphor persistence effect.
//!
//! Renders PCM waveform data from the onboard MEMS microphone as a green
//! oscilloscope trace across the 720×480 landscape viewport. Three history
//! buffers provide a phosphor decay effect with decreasing intensity.
//!
//! ## SDRAM buffer layout (shared with star crawl at [`SCOPE_BASE`])
//!
//! | Buffer       | Size          | Format   | Purpose                        |
//! |-------------|---------------|----------|--------------------------------|
//! | Scope frame | 720×480×4     | ARGB8888 | Per-frame composited output    |

#![allow(dead_code)]

/// Landscape width of the scope drawing area (excluding icon bar).
const SCOPE_W: u32 = 720;
/// Landscape height of the scope drawing area.
const SCOPE_H: u32 = 480;
/// ARGB8888 bytes per pixel.
const BPP: u32 = 4;
/// Number of PCM samples per frame (one per pixel column).
const SAMPLES: usize = 720;
/// SDRAM base address for scope frame buffer (shared with star crawl).
const SCOPE_BASE: usize = 0xD048_0000;
/// Background color (near-black).
const BG_COLOR: u32 = 0xFF0A_0A0A;
/// Vertical center of the scope (y = 240).
const CENTER_Y: i32 = (SCOPE_H / 2) as i32;

/// Green intensity for each trace (center pixel), oldest to newest.
const TRACE_GREEN: [u8; 4] = [0x40, 0x80, 0xC0, 0xFF];

/// Audio oscilloscope state machine.
pub struct AudioScope {
    active: bool,
    /// Pointer to 720×480 ARGB8888 landscape working buffer in SDRAM.
    scope_buf: *mut u8,
    /// Ring of 4 waveform buffers: current + 3 history.
    waveforms: [[i16; SAMPLES]; 4],
    /// Index of the current (newest) waveform in the ring.
    waveform_idx: usize,
}

impl AudioScope {
    /// Create a new scope (inactive).
    pub const fn new() -> Self {
        Self {
            active: false,
            scope_buf: core::ptr::null_mut(),
            waveforms: [[0i16; SAMPLES]; 4],
            waveform_idx: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate the scope: set up SDRAM buffer pointer, clear state.
    pub fn activate(&mut self) {
        self.scope_buf = SCOPE_BASE as *mut u8;
        self.waveforms = [[0i16; SAMPLES]; 4];
        self.waveform_idx = 0;
        self.active = true;
    }

    /// Deactivate the scope.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Render one frame of the oscilloscope.
    ///
    /// `samples` must contain exactly 720 PCM samples (i16).
    /// Renders to the intermediate landscape buffer, then rotates into the
    /// portrait back buffer. Returns `true` if a frame was produced.
    pub fn tick(
        &mut self,
        samples: &[i16],
        back_buf: *mut u8,
        fb_w: u32,
        fb_h: u32,
    ) -> bool {
        if !self.active || self.scope_buf.is_null() {
            return false;
        }

        // Advance ring and store new samples
        self.waveform_idx = (self.waveform_idx + 1) % 4;
        let idx = self.waveform_idx;
        for i in 0..SAMPLES {
            self.waveforms[idx][i] = if i < samples.len() { samples[i] } else { 0 };
        }

        // Clear scope buffer to background
        let pixel_count = (SCOPE_W * SCOPE_H) as usize;
        let buf32 = self.scope_buf as *mut u32;
        unsafe {
            for i in 0..pixel_count {
                buf32.add(i).write_volatile(BG_COLOR);
            }
        }

        // Draw 4 traces, oldest first so newest overwrites at overlaps
        for age in 0..4u8 {
            // age 0 = oldest (3 frames ago), age 3 = current
            let ring_idx = (self.waveform_idx + 1 + age as usize) % 4;
            let green_center = TRACE_GREEN[age as usize];
            let green_mid = green_center / 2;
            let green_outer = green_center / 4;

            let waveform = &self.waveforms[ring_idx];
            let stride = SCOPE_W as usize;

            for x in 0..SAMPLES {
                // Scale sample to pixel offset from center.
                // i16 range is -32768..32767 → map to -240..+240 pixels.
                let sample = waveform[x] as i32;
                let y_offset = (sample * CENTER_Y) / 32768;
                let y_center = CENTER_Y - y_offset; // invert: positive sample → up

                // Draw 5-pixel vertical spread at this x column
                let spreads: [(i32, u8); 5] = [
                    (y_center - 2, green_outer),
                    (y_center - 1, green_mid),
                    (y_center,     green_center),
                    (y_center + 1, green_mid),
                    (y_center + 2, green_outer),
                ];

                for &(y, green) in &spreads {
                    if y >= 0 && y < SCOPE_H as i32 {
                        let pixel_idx = y as usize * stride + x;
                        let argb = 0xFF00_0000 | ((green as u32) << 8);
                        unsafe {
                            buf32.add(pixel_idx).write_volatile(argb);
                        }
                    }
                }
            }
        }

        // Rotate landscape buffer → portrait back buffer
        self.rotate_to_portrait(back_buf, fb_w, fb_h);

        true
    }

    /// Copy landscape scope frame → portrait back buffer with 90° rotation.
    ///
    /// Landscape (720×480 ARGB8888) → Portrait (480×800 ARGB8888).
    /// Only writes the left 720 columns (portrait rows 0..719), leaving
    /// the icon bar untouched.
    fn rotate_to_portrait(&self, dst: *mut u8, fb_w: u32, _fb_h: u32) {
        let src_stride = SCOPE_W * BPP;
        let dst_stride = fb_w * BPP;

        for ly in 0..SCOPE_H {
            let src_row = unsafe { self.scope_buf.add((ly * src_stride) as usize) };
            // Landscape (lx, ly) → portrait (fb_w - 1 - ly, lx)
            let dst_col = fb_w - 1 - ly;

            for lx in 0..SCOPE_W {
                let dst_offset = (lx * dst_stride + dst_col * BPP) as usize;
                unsafe {
                    let pixel = (src_row.add((lx * BPP) as usize) as *const u32).read_volatile();
                    (dst.add(dst_offset) as *mut u32).write_volatile(pixel);
                }
            }
        }
    }
}
