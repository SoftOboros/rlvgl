//! Audio oscilloscope visualization rendered as a stepped task.
//!
//! The scope advances in small chunks so the CM7 round-robin loop stays
//! responsive while a frame is being cleared, drawn, and rotated.

/// Landscape width of the scope drawing area.
const SCOPE_W: u32 = 720;
/// Landscape height of the scope drawing area.
const SCOPE_H: u32 = 480;
/// ARGB8888 bytes per pixel.
const BPP: u32 = 4;
/// Number of PCM samples per frame.
const SAMPLES: usize = 720;
/// SDRAM base address for the scope working buffer.
const SCOPE_BASE: usize = 0xD048_0000;
/// Background colour.
const BG_COLOR: u32 = 0xFF0A_0A0A;
/// Vertical centre of the scope.
const CENTER_Y: i32 = (SCOPE_H / 2) as i32;
/// Samples processed per scheduler step.
const STEP_SAMPLES: usize = 32;
/// Rows cleared per scheduler step.
const CLEAR_ROWS_PER_STEP: u32 = 16;

/// Green intensity for each trace, oldest to newest.
const TRACE_GREEN: [u8; 4] = [0x40, 0x80, 0xC0, 0xFF];

#[derive(Copy, Clone, Eq, PartialEq)]
enum ScopeStage {
    Idle = 0,
    Clear = 1,
    Draw = 2,
    Rotate = 3,
}

/// Result of a single scope scheduler step.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum StepResult {
    /// Scope is inactive.
    Idle,
    /// More work remains before the back buffer can be presented.
    Pending,
    /// The frame is complete and ready to present.
    FrameReady,
}

/// Audio oscilloscope state machine.
pub struct AudioScope {
    active: bool,
    scope_buf: *mut u8,
    waveforms: [[i16; SAMPLES]; 4],
    waveform_idx: usize,
    frame_id: u32,
    frame_active: bool,
    clear_row: u32,
    draw_x: usize,
    rotate_x: u32,
    back_buf: *mut u8,
    fb_w: u32,
    stage: ScopeStage,
}

impl AudioScope {
    /// Create a new scope (inactive).
    pub const fn new() -> Self {
        Self {
            active: false,
            scope_buf: core::ptr::null_mut(),
            waveforms: [[0i16; SAMPLES]; 4],
            waveform_idx: 0,
            frame_id: 0,
            frame_active: false,
            clear_row: 0,
            draw_x: 0,
            rotate_x: 0,
            back_buf: core::ptr::null_mut(),
            fb_w: 0,
            stage: ScopeStage::Idle,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate the scope and reset frame state.
    pub fn activate(&mut self) {
        // SCOPE_BASE is a fixed D2 SRAM scratch region for the audio
        // scope's pixel buffer (avoids SDRAM bus contention with LTDC).
        self.scope_buf = SCOPE_BASE as *mut u8; // rlvgl-discipline: allow(raw_addr_cast)
        self.waveforms = [[0i16; SAMPLES]; 4];
        self.waveform_idx = 0;
        self.frame_id = 0;
        self.active = true;
        self.frame_active = false;
        self.stage = ScopeStage::Idle;
    }

    /// Deactivate the scope.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.frame_active = false;
        self.stage = ScopeStage::Idle;
    }

    /// Numeric stage identifier for telemetry.
    pub fn stage_code(&self) -> u32 {
        self.stage as u32
    }

    /// Current frame identifier for telemetry.
    pub fn frame_id(&self) -> u32 {
        self.frame_id
    }

    /// Drop the current in-progress frame.
    pub fn drop_frame(&mut self) {
        self.frame_active = false;
        self.stage = ScopeStage::Idle;
        self.clear_row = 0;
        self.draw_x = 0;
        self.rotate_x = 0;
    }

    /// Advance the scope renderer by one bounded step.
    pub fn tick(
        &mut self,
        samples: &[i16],
        back_buf: *mut u8,
        fb_w: u32,
        _fb_h: u32,
    ) -> StepResult {
        if !self.active || self.scope_buf.is_null() {
            return StepResult::Idle;
        }

        if !self.frame_active {
            self.frame_id = self.frame_id.wrapping_add(1);
            self.frame_active = true;
            self.clear_row = 0;
            self.draw_x = 0;
            self.rotate_x = 0;
            self.back_buf = back_buf;
            self.fb_w = fb_w;
            self.stage = ScopeStage::Clear;

            self.waveform_idx = (self.waveform_idx + 1) % 4;
            let waveform = &mut self.waveforms[self.waveform_idx];
            for (i, slot) in waveform.iter_mut().enumerate() {
                *slot = samples.get(i).copied().unwrap_or(0);
            }
        }

        match self.stage {
            ScopeStage::Idle => StepResult::Pending,
            ScopeStage::Clear => {
                self.clear_rows();
                if self.clear_row >= SCOPE_H {
                    self.stage = ScopeStage::Draw;
                }
                StepResult::Pending
            }
            ScopeStage::Draw => {
                self.draw_chunk();
                if self.draw_x >= SAMPLES {
                    self.stage = ScopeStage::Rotate;
                }
                StepResult::Pending
            }
            ScopeStage::Rotate => {
                self.rotate_chunk();
                if self.rotate_x >= SCOPE_W {
                    self.drop_frame();
                    StepResult::FrameReady
                } else {
                    StepResult::Pending
                }
            }
        }
    }

    fn clear_rows(&mut self) {
        let buf32 = self.scope_buf.cast::<u32>();
        let stride = SCOPE_W as usize;
        let end = (self.clear_row + CLEAR_ROWS_PER_STEP).min(SCOPE_H);
        for row in self.clear_row..end {
            let row_ptr = unsafe { buf32.add(row as usize * stride) };
            for col in 0..stride {
                unsafe {
                    row_ptr.add(col).write_volatile(BG_COLOR);
                }
            }
        }
        self.clear_row = end;
    }

    fn draw_chunk(&mut self) {
        let start = self.draw_x;
        let end = (start + STEP_SAMPLES).min(SAMPLES);
        let stride = SCOPE_W as usize;
        let buf32 = self.scope_buf.cast::<u32>();

        for age in 0..4u8 {
            let ring_idx = (self.waveform_idx + 1 + age as usize) % 4;
            let green_center = TRACE_GREEN[age as usize];
            let green_mid = green_center / 2;
            let green_outer = green_center / 4;
            let waveform = &self.waveforms[ring_idx];

            for x in start..end {
                let sample = waveform[x] as i32;
                let y_offset = (sample * CENTER_Y) / 32768;
                let y_center = CENTER_Y - y_offset;

                let spreads = [
                    (y_center - 2, green_outer),
                    (y_center - 1, green_mid),
                    (y_center, green_center),
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

        self.draw_x = end;
    }

    fn rotate_chunk(&mut self) {
        let src_stride = SCOPE_W * BPP;
        let dst_stride = self.fb_w * BPP;
        let end = (self.rotate_x + STEP_SAMPLES as u32).min(SCOPE_W);

        for lx in self.rotate_x..end {
            for ly in 0..SCOPE_H {
                let dst_col = self.fb_w - 1 - ly;
                let dst_offset = (lx * dst_stride + dst_col * BPP) as usize;
                let src_offset = (ly * src_stride + lx * BPP) as usize;
                unsafe {
                    let pixel = self.scope_buf.add(src_offset).cast::<u32>().read_volatile();
                    self.back_buf
                        .add(dst_offset)
                        .cast::<u32>()
                        .write_volatile(pixel);
                }
            }
        }

        self.rotate_x = end;
    }
}
