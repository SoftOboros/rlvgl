// SPDX-License-Identifier: MIT
//! Host-independent WLD geometry, damage, pixel, and pacing state.

#[cfg(test)]
use alloc::vec;
use alloc::vec::Vec;
use core::mem::size_of;

use rlvgl_core::widget::{Color, Rect};

const BYTES_PER_XRGB8888_PIXEL: usize = 4;
const SCTK_SLOT_ALIGNMENT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelError {
    InvalidConfig(&'static str),
    GeometryOverflow,
    SurfaceTooSmall {
        surface: (u32, u32),
        canvas: (u32, u32),
    },
    PixelLength {
        expected: usize,
        actual: usize,
    },
}

/// Adopted logical, surface, and buffer geometry for one SHM generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
    pub(crate) logical_width: u32,
    pub(crate) logical_height: u32,
    pub(crate) surface_width: u32,
    pub(crate) surface_height: u32,
    pub(crate) scale: u32,
    pub(crate) canvas_x: u32,
    pub(crate) canvas_y: u32,
    pub(crate) buffer_width: i32,
    pub(crate) buffer_height: i32,
    pub(crate) stride: i32,
    pub(crate) slot_bytes: usize,
    pub(crate) aligned_slot_bytes: usize,
    pub(crate) shadow_bytes: usize,
}

impl Geometry {
    pub(crate) fn checked(
        logical_size: (u32, u32),
        surface_size: (u32, u32),
        scale: u32,
    ) -> Result<Self, ModelError> {
        let (logical_width, logical_height) = logical_size;
        let (surface_width, surface_height) = surface_size;
        if logical_width == 0 || logical_height == 0 {
            return Err(ModelError::InvalidConfig(
                "logical width and height must be nonzero",
            ));
        }
        if surface_width < logical_width || surface_height < logical_height {
            return Err(ModelError::SurfaceTooSmall {
                surface: surface_size,
                canvas: logical_size,
            });
        }
        if scale == 0 {
            return Err(ModelError::InvalidConfig(
                "Wayland buffer scale must be positive",
            ));
        }

        let buffer_width_u32 = surface_width
            .checked_mul(scale)
            .ok_or(ModelError::GeometryOverflow)?;
        let buffer_height_u32 = surface_height
            .checked_mul(scale)
            .ok_or(ModelError::GeometryOverflow)?;
        let buffer_width =
            i32::try_from(buffer_width_u32).map_err(|_| ModelError::GeometryOverflow)?;
        let buffer_height =
            i32::try_from(buffer_height_u32).map_err(|_| ModelError::GeometryOverflow)?;
        let stride_usize = usize::try_from(buffer_width)
            .map_err(|_| ModelError::GeometryOverflow)?
            .checked_mul(BYTES_PER_XRGB8888_PIXEL)
            .ok_or(ModelError::GeometryOverflow)?;
        let stride = i32::try_from(stride_usize).map_err(|_| ModelError::GeometryOverflow)?;
        let slot_bytes = stride_usize
            .checked_mul(usize::try_from(buffer_height).map_err(|_| ModelError::GeometryOverflow)?)
            .ok_or(ModelError::GeometryOverflow)?;
        let aligned_slot_bytes = align_slot(slot_bytes)?;
        let shadow_bytes = usize::try_from(logical_width)
            .map_err(|_| ModelError::GeometryOverflow)?
            .checked_mul(usize::try_from(logical_height).map_err(|_| ModelError::GeometryOverflow)?)
            .and_then(|pixels| pixels.checked_mul(size_of::<Color>()))
            .ok_or(ModelError::GeometryOverflow)?;

        Ok(Self {
            logical_width,
            logical_height,
            surface_width,
            surface_height,
            scale,
            canvas_x: (surface_width - logical_width) / 2,
            canvas_y: (surface_height - logical_height) / 2,
            buffer_width,
            buffer_height,
            stride,
            slot_bytes,
            aligned_slot_bytes,
            shadow_bytes,
        })
    }

    pub(crate) fn steady_bytes(self, slot_count: usize) -> Result<usize, ModelError> {
        self.aligned_slot_bytes
            .checked_mul(slot_count)
            .and_then(|slots| slots.checked_add(self.shadow_bytes))
            .ok_or(ModelError::GeometryOverflow)
    }

    pub(crate) fn shadow_len(self) -> Result<usize, ModelError> {
        usize::try_from(self.logical_width)
            .map_err(|_| ModelError::GeometryOverflow)?
            .checked_mul(
                usize::try_from(self.logical_height).map_err(|_| ModelError::GeometryOverflow)?,
            )
            .ok_or(ModelError::GeometryOverflow)
    }

    pub(crate) fn full_buffer_damage(self) -> (i32, i32, i32, i32) {
        (0, 0, self.buffer_width, self.buffer_height)
    }

    pub(crate) fn map_damage(self, rect: Rect) -> Option<(i32, i32, i32, i32)> {
        let x0 = i64::from(rect.x.max(0)).min(i64::from(self.logical_width));
        let y0 = i64::from(rect.y.max(0)).min(i64::from(self.logical_height));
        let x1 =
            i64::from(rect.x.saturating_add(rect.width).max(0)).min(i64::from(self.logical_width));
        let y1 = i64::from(rect.y.saturating_add(rect.height).max(0))
            .min(i64::from(self.logical_height));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }

        let scale = i64::from(self.scale);
        let bx = (x0 + i64::from(self.canvas_x)).checked_mul(scale)?;
        let by = (y0 + i64::from(self.canvas_y)).checked_mul(scale)?;
        let bw = (x1 - x0).checked_mul(scale)?;
        let bh = (y1 - y0).checked_mul(scale)?;
        Some((
            i32::try_from(bx).ok()?,
            i32::try_from(by).ok()?,
            i32::try_from(bw).ok()?,
            i32::try_from(bh).ok()?,
        ))
    }
}

fn align_slot(bytes: usize) -> Result<usize, ModelError> {
    bytes
        .checked_add(SCTK_SLOT_ALIGNMENT - 1)
        .map(|value| value & !(SCTK_SLOT_ALIGNMENT - 1))
        .ok_or(ModelError::GeometryOverflow)
}

pub(crate) fn allocation_peak(
    steady_bytes: usize,
    retired_bytes: usize,
) -> Result<usize, ModelError> {
    steady_bytes
        .checked_add(retired_bytes)
        .ok_or(ModelError::GeometryOverflow)
}

pub(crate) const fn slot_count_is_valid(count: u8) -> bool {
    matches!(count, 2 | 3)
}

pub(crate) fn resolve_configure_geometry(
    adaptive: bool,
    fixed_logical_size: (u32, u32),
    suggested: (Option<u32>, Option<u32>),
    fallback_surface_size: (u32, u32),
) -> ((u32, u32), (u32, u32)) {
    let surface_size = (
        suggested.0.unwrap_or(fallback_surface_size.0),
        suggested.1.unwrap_or(fallback_surface_size.1),
    );
    let logical_size = if adaptive {
        surface_size
    } else {
        fixed_logical_size
    };
    (logical_size, surface_size)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AllocationAdmission {
    Ready { peak_bytes: usize },
    SteadyTooLarge { required: usize },
    Deferred { required: usize },
}

pub(crate) fn classify_allocation(
    steady_bytes: usize,
    retired_bytes: usize,
    active_retirement_bytes: usize,
    limit: usize,
) -> Result<AllocationAdmission, ModelError> {
    if steady_bytes > limit {
        return Ok(AllocationAdmission::SteadyTooLarge {
            required: steady_bytes,
        });
    }
    let old_generation_bytes = allocation_peak(retired_bytes, active_retirement_bytes)?;
    let peak_bytes = allocation_peak(steady_bytes, old_generation_bytes)?;
    if peak_bytes > limit {
        Ok(AllocationAdmission::Deferred {
            required: peak_bytes,
        })
    } else {
        Ok(AllocationAdmission::Ready { peak_bytes })
    }
}

#[derive(Debug)]
pub(crate) struct DamageSet {
    pub(crate) rects: Vec<Rect>,
    limit: usize,
    pub(crate) full: bool,
}

impl DamageSet {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            rects: Vec::with_capacity(limit),
            limit,
            full: false,
        }
    }

    pub(crate) fn add(&mut self, rect: Rect) {
        if self.full {
            return;
        }
        if self.rects.len() == self.limit {
            self.mark_full();
            return;
        }
        self.rects.push(rect);
    }

    pub(crate) fn mark_full(&mut self) {
        self.rects.clear();
        self.full = true;
    }

    pub(crate) fn clear(&mut self) {
        self.rects.clear();
        self.full = false;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SubmissionState {
    pending_present: bool,
    frame_gate_open: bool,
    stopped: bool,
    coalesced_presents: u64,
}

impl SubmissionState {
    pub(crate) const fn new() -> Self {
        Self {
            pending_present: false,
            frame_gate_open: true,
            stopped: false,
            coalesced_presents: 0,
        }
    }

    pub(crate) fn request_present(&mut self) {
        if self.stopped {
            return;
        }
        if self.pending_present {
            self.coalesced_presents = self.coalesced_presents.saturating_add(1);
        }
        self.pending_present = true;
    }

    pub(crate) const fn can_probe_slot(self) -> bool {
        !self.stopped && self.pending_present && self.frame_gate_open
    }

    pub(crate) fn submitted(&mut self) {
        debug_assert!(self.can_probe_slot());
        self.frame_gate_open = false;
        self.pending_present = false;
    }

    pub(crate) fn frame_done(&mut self) {
        if !self.stopped {
            self.frame_gate_open = true;
        }
    }

    pub(crate) fn stop(&mut self) {
        self.stopped = true;
        self.pending_present = false;
    }

    pub(crate) fn reset_for_geometry(&mut self) {
        if !self.stopped {
            self.pending_present = false;
        }
    }

    pub(crate) const fn coalesced_presents(self) -> u64 {
        self.coalesced_presents
    }
}

pub(crate) fn encode_complete_frame(
    geometry: Geometry,
    shadow: &[Color],
    destination: &mut [u8],
) -> Result<(), ModelError> {
    let expected_shadow = geometry.shadow_len()?;
    if shadow.len() != expected_shadow {
        return Err(ModelError::PixelLength {
            expected: expected_shadow,
            actual: shadow.len(),
        });
    }
    if destination.len() < geometry.slot_bytes {
        return Err(ModelError::PixelLength {
            expected: geometry.slot_bytes,
            actual: destination.len(),
        });
    }

    destination[..geometry.slot_bytes]
        .chunks_exact_mut(BYTES_PER_XRGB8888_PIXEL)
        .for_each(|pixel| pixel.copy_from_slice(&[0, 0, 0, 0xff]));

    let scale = usize::try_from(geometry.scale).map_err(|_| ModelError::GeometryOverflow)?;
    let buffer_width =
        usize::try_from(geometry.buffer_width).map_err(|_| ModelError::GeometryOverflow)?;
    let logical_width =
        usize::try_from(geometry.logical_width).map_err(|_| ModelError::GeometryOverflow)?;
    let logical_height =
        usize::try_from(geometry.logical_height).map_err(|_| ModelError::GeometryOverflow)?;
    let canvas_x = usize::try_from(geometry.canvas_x).map_err(|_| ModelError::GeometryOverflow)?;
    let canvas_y = usize::try_from(geometry.canvas_y).map_err(|_| ModelError::GeometryOverflow)?;

    for logical_y in 0..logical_height {
        for logical_x in 0..logical_width {
            let Color(red, green, blue, _alpha) = shadow[logical_y * logical_width + logical_x];
            let bytes = [blue, green, red, 0xff];
            let base_x = (canvas_x + logical_x) * scale;
            let base_y = (canvas_y + logical_y) * scale;
            for sy in 0..scale {
                let row_start = ((base_y + sy) * buffer_width + base_x) * BYTES_PER_XRGB8888_PIXEL;
                for sx in 0..scale {
                    let offset = row_start + sx * BYTES_PER_XRGB8888_PIXEL;
                    destination[offset..offset + BYTES_PER_XRGB8888_PIXEL].copy_from_slice(&bytes);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_pixel(x: i32) -> Rect {
        Rect {
            x,
            y: 0,
            width: 1,
            height: 1,
        }
    }

    #[test]
    fn checked_geometry_accounts_for_scale_slots_and_shadow() {
        let geometry = Geometry::checked((2, 1), (4, 3), 2).unwrap();
        assert_eq!(geometry.canvas_x, 1);
        assert_eq!(geometry.canvas_y, 1);
        assert_eq!(geometry.buffer_width, 8);
        assert_eq!(geometry.buffer_height, 6);
        assert_eq!(geometry.stride, 32);
        assert_eq!(geometry.slot_bytes, 192);
        assert_eq!(geometry.aligned_slot_bytes, 192);
        assert_eq!(geometry.shadow_bytes, 8);
        assert_eq!(geometry.steady_bytes(3).unwrap(), 584);
    }

    #[test]
    fn undersized_surface_is_rejected_without_crop_or_scale() {
        assert!(matches!(
            Geometry::checked((800, 480), (799, 480), 1),
            Err(ModelError::SurfaceTooSmall { .. })
        ));
    }

    #[test]
    fn xrgb_encoding_scales_canvas_and_fills_opaque_letterbox() {
        let geometry = Geometry::checked((2, 1), (4, 3), 2).unwrap();
        let shadow = [Color(1, 2, 3, 4), Color(10, 20, 30, 40)];
        let mut bytes = vec![0xaa; geometry.slot_bytes];
        encode_complete_frame(geometry, &shadow, &mut bytes).unwrap();

        let pixel = |x: usize, y: usize| {
            let offset = (y * 8 + x) * 4;
            &bytes[offset..offset + 4]
        };
        assert_eq!(pixel(0, 0), [0, 0, 0, 0xff]);
        assert_eq!(pixel(2, 2), [3, 2, 1, 0xff]);
        assert_eq!(pixel(3, 3), [3, 2, 1, 0xff]);
        assert_eq!(pixel(4, 2), [30, 20, 10, 0xff]);
        assert_eq!(pixel(7, 5), [0, 0, 0, 0xff]);
    }

    #[test]
    fn complete_copy_prevents_stale_pixels_beyond_slot_rotation() {
        let geometry = Geometry::checked((3, 1), (3, 1), 1).unwrap();
        let mut slots = [
            vec![0; geometry.slot_bytes],
            vec![0; geometry.slot_bytes],
            vec![0; geometry.slot_bytes],
        ];
        for frame in 0..8u8 {
            let shadow = [
                Color(frame, 0, 0, 0),
                Color(0, frame, 0, 0),
                Color(0, 0, frame, 0),
            ];
            let slot = &mut slots[usize::from(frame) % 3];
            encode_complete_frame(geometry, &shadow, slot).unwrap();
            assert_eq!(slot[0..4], [0, 0, frame, 0xff]);
            assert_eq!(slot[4..8], [0, frame, 0, 0xff]);
            assert_eq!(slot[8..12], [frame, 0, 0, 0xff]);
        }
    }

    #[test]
    fn logical_damage_maps_once_into_scaled_buffer_coordinates() {
        let geometry = Geometry::checked((4, 3), (8, 5), 2).unwrap();
        assert_eq!(geometry.full_buffer_damage(), (0, 0, 16, 10));
        assert_eq!(
            geometry.map_damage(Rect {
                x: 1,
                y: 1,
                width: 2,
                height: 1,
            }),
            Some((6, 4, 4, 2))
        );
    }

    #[test]
    fn geometry_and_peak_allocation_overflow_are_typed() {
        assert!(matches!(
            Geometry::checked((u32::MAX, 1), (u32::MAX, 1), 2),
            Err(ModelError::GeometryOverflow)
        ));
        assert_eq!(
            allocation_peak(usize::MAX, 1),
            Err(ModelError::GeometryOverflow)
        );
    }

    #[test]
    fn resize_budget_defers_only_until_old_generations_release() {
        assert_eq!(
            classify_allocation(60, 30, 20, 100).unwrap(),
            AllocationAdmission::Deferred { required: 110 }
        );
        assert_eq!(
            classify_allocation(60, 0, 0, 100).unwrap(),
            AllocationAdmission::Ready { peak_bytes: 60 }
        );
        assert_eq!(
            classify_allocation(101, 0, 0, 100).unwrap(),
            AllocationAdmission::SteadyTooLarge { required: 101 }
        );
    }

    #[test]
    fn slot_count_contract_is_exact() {
        assert!(!slot_count_is_valid(1));
        assert!(slot_count_is_valid(2));
        assert!(slot_count_is_valid(3));
        assert!(!slot_count_is_valid(4));
    }

    #[test]
    fn adaptive_configure_adopts_smaller_surface_at_boundary() {
        assert_eq!(
            resolve_configure_geometry(true, (800, 480), (Some(640), Some(360)), (800, 480)),
            ((640, 360), (640, 360))
        );
    }

    #[test]
    fn zero_configure_dimensions_retain_current_dimension() {
        assert_eq!(
            resolve_configure_geometry(true, (800, 480), (None, Some(600)), (1024, 768)),
            ((1024, 600), (1024, 600))
        );
    }

    #[test]
    fn fixed_canvas_retains_logical_size_inside_larger_surface() {
        assert_eq!(
            resolve_configure_geometry(false, (800, 480), (Some(1920), Some(1080)), (800, 480)),
            ((800, 480), (1920, 1080))
        );
    }

    #[test]
    fn bounded_damage_promotes_to_full_at_overflow() {
        let mut damage = DamageSet::new(2);
        damage.add(one_pixel(0));
        damage.add(one_pixel(1));
        assert!(!damage.full);
        damage.add(one_pixel(2));
        assert!(damage.full);
        assert!(damage.rects.is_empty());
        damage.clear();
        assert!(!damage.full);
    }

    #[test]
    fn frame_before_release_keeps_latest_present_pending() {
        let mut state = SubmissionState::new();
        state.request_present();
        assert!(state.can_probe_slot());
        state.submitted();
        state.request_present();
        state.frame_done();
        assert!(state.can_probe_slot());
        assert_eq!(state.coalesced_presents(), 0);
    }

    #[test]
    fn release_before_frame_does_not_open_pacing_gate() {
        let mut state = SubmissionState::new();
        state.request_present();
        state.submitted();
        state.request_present();
        assert!(!state.can_probe_slot());
        state.frame_done();
        assert!(state.can_probe_slot());
    }

    #[test]
    fn all_slots_busy_coalesces_without_allocating_state() {
        let mut state = SubmissionState::new();
        state.request_present();
        state.request_present();
        state.request_present();
        assert!(state.can_probe_slot());
        assert_eq!(state.coalesced_presents(), 2);
        assert_eq!(size_of::<SubmissionState>(), size_of::<(u64, u32)>());
    }

    #[test]
    fn terminal_stop_closes_both_submission_gates() {
        let mut state = SubmissionState::new();
        state.request_present();
        state.stop();
        state.frame_done();
        state.request_present();
        assert!(!state.can_probe_slot());
    }

    #[test]
    fn geometry_change_drops_only_the_stale_pending_present() {
        let mut state = SubmissionState::new();
        state.request_present();
        state.reset_for_geometry();
        assert!(!state.can_probe_slot());
        state.request_present();
        assert!(state.can_probe_slot());
    }
}
