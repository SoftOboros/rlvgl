// SPDX-License-Identifier: MIT
//! Shadow-frame display adapter and bounded Wayland submission state.

use alloc::{rc::Rc, string::ToString, vec, vec::Vec};
use core::{cell::RefCell, mem::size_of};

use rlvgl_core::widget::{Color, Rect};
use smithay_client_toolkit::{
    compositor::{CompositorState, FrameCallbackData, Region},
    shell::{WaylandSurface, xdg::window::Window},
};
use wayland_client::{QueueHandle, protocol::wl_output};

use crate::{
    display::DisplayDriver,
    screen::{Rotation, Screen},
};

use super::model::{
    AllocationAdmission, DamageSet, Geometry, SubmissionState, classify_allocation,
};
use super::{ProtocolState, WaylandConfig, WaylandError, shm::ShmGeneration};

/// Snapshot of bounded WLD-01 presentation state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WaylandDisplayStats {
    /// Number of frames successfully committed.
    pub submitted_frames: u64,
    /// Number of compositor frame callbacks observed after submission.
    pub frame_callbacks: u64,
    /// Number of present requests coalesced behind a closed pacing or slot gate.
    pub coalesced_presents: u64,
    /// Bytes currently charged to the Shadow Frame and SHM generations.
    pub allocated_bytes: usize,
    /// Number of release-tracked retired resize generations.
    pub retired_generations: usize,
}

/// `DisplayDriver` adapter backed by a private complete Shadow Frame.
///
/// The adapter is borrowed through its owning [`super::WaylandSession`] and is
/// intentionally not cloneable or independently dispatchable.
pub struct WaylandDisplay {
    pub(crate) presenter: Rc<RefCell<Presenter>>,
}

impl core::fmt::Debug for WaylandDisplay {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("WaylandDisplay")
            .field("screen", &self.presenter.borrow().screen)
            .finish_non_exhaustive()
    }
}

impl WaylandDisplay {
    /// Return current bounded presentation telemetry.
    pub fn stats(&self) -> WaylandDisplayStats {
        self.presenter.borrow_mut().stats()
    }
}

impl DisplayDriver for WaylandDisplay {
    fn screen(&self) -> Screen {
        self.presenter.borrow().screen
    }

    fn flush(&mut self, area: Rect, colors: &[Color]) {
        let mut presenter = self.presenter.borrow_mut();
        if let Err(error) = presenter.flush(area, colors) {
            presenter.record_error(error);
        }
    }

    fn vsync(&mut self) {
        let mut presenter = self.presenter.borrow_mut();
        presenter.request_present();
        if let Err(error) = presenter.try_submit() {
            presenter.record_error(error);
        }
    }
}

pub(crate) struct Presenter {
    qh: QueueHandle<ProtocolState>,
    window: Window,
    compositor: CompositorState,
    opaque_region: Option<Region>,
    slot_count: usize,
    max_allocation_bytes: usize,
    screen: Screen,
    geometry: Option<Geometry>,
    shadow: Vec<Color>,
    damage: DamageSet,
    active: Option<ShmGeneration>,
    retired: Vec<ShmGeneration>,
    submission: SubmissionState,
    last_error: Option<WaylandError>,
    submitted_frames: u64,
    frame_callbacks: u64,
}

impl core::fmt::Debug for Presenter {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Presenter")
            .field("screen", &self.screen)
            .field("geometry", &self.geometry)
            .field("slot_count", &self.slot_count)
            .field("retired_generations", &self.retired.len())
            .field("submission", &self.submission)
            .finish_non_exhaustive()
    }
}

impl Presenter {
    pub(crate) fn new(
        config: &WaylandConfig,
        qh: QueueHandle<ProtocolState>,
        window: Window,
        compositor: CompositorState,
    ) -> Self {
        let initial_size = config.initial_size;
        Self {
            qh,
            window,
            compositor,
            opaque_region: None,
            slot_count: usize::from(config.buffer_count.get()),
            max_allocation_bytes: config.limits.max_allocation_bytes.get(),
            screen: Screen::new(initial_size.0, initial_size.1, Rotation::Deg0),
            geometry: None,
            shadow: Vec::new(),
            damage: DamageSet::new(config.limits.max_damage_rects.get()),
            active: None,
            retired: Vec::new(),
            submission: SubmissionState::new(),
            last_error: None,
            submitted_frames: 0,
            frame_callbacks: 0,
        }
    }

    pub(crate) fn accept_geometry(
        &mut self,
        geometry: Geometry,
        shm: &smithay_client_toolkit::shm::Shm,
    ) -> Result<(), WaylandError> {
        self.release_retired();
        if self.geometry == Some(geometry) && self.active.is_some() {
            return Ok(());
        }

        // The renderer has crossed the configure boundary. Old-size content
        // must no longer attach; Busy slots retire until release and the old
        // Shadow Frame can be discarded immediately.
        self.retire_active();
        self.geometry = None;
        self.shadow = Vec::new();
        self.opaque_region = None;
        self.damage.mark_full();
        self.submission.reset_for_geometry();

        let steady_bytes = geometry.steady_bytes(self.slot_count)?;
        let retired_bytes = self.retired_pool_bytes()?;
        match classify_allocation(steady_bytes, retired_bytes, 0, self.max_allocation_bytes)? {
            AllocationAdmission::Ready { .. } => {}
            AllocationAdmission::SteadyTooLarge { required } => {
                return Err(WaylandError::GeometryTooLarge {
                    required,
                    limit: self.max_allocation_bytes,
                });
            }
            AllocationAdmission::Deferred { required } => {
                return Err(WaylandError::AllocationDeferred {
                    required,
                    limit: self.max_allocation_bytes,
                });
            }
        }

        let generation = ShmGeneration::new(geometry, self.slot_count, shm)?;
        let shadow = vec![Color(0, 0, 0, 0xff); geometry.shadow_len()?];
        let region = Region::new(&self.compositor)
            .map_err(|error| WaylandError::Protocol(error.to_string()))?;
        region.add(
            0,
            0,
            i32::try_from(geometry.surface_width).map_err(|_| WaylandError::GeometryOverflow)?,
            i32::try_from(geometry.surface_height).map_err(|_| WaylandError::GeometryOverflow)?,
        );

        let scale = i32::try_from(geometry.scale).map_err(|_| WaylandError::GeometryOverflow)?;
        let surface = self.window.wl_surface();
        surface.set_buffer_scale(scale);
        surface.set_buffer_transform(wl_output::Transform::Normal);
        surface.set_opaque_region(Some(region.wl_region()));

        self.screen = Screen::new(
            geometry.logical_width,
            geometry.logical_height,
            Rotation::Deg0,
        );
        self.geometry = Some(geometry);
        self.shadow = shadow;
        self.active = Some(generation);
        self.opaque_region = Some(region);
        Ok(())
    }

    pub(crate) fn frame_done(&mut self) -> Result<(), WaylandError> {
        self.frame_callbacks = self.frame_callbacks.saturating_add(1);
        self.submission.frame_done();
        self.try_submit()
    }

    pub(crate) fn progress_after_dispatch(&mut self) -> Result<(), WaylandError> {
        self.release_retired();
        self.try_submit()
    }

    pub(crate) fn take_error(&mut self) -> Option<WaylandError> {
        self.last_error.take()
    }

    pub(crate) fn record_error(&mut self, error: WaylandError) {
        if self.last_error.is_none() {
            self.last_error = Some(error);
        }
    }

    pub(crate) fn stop(&mut self) {
        self.submission.stop();
    }

    fn flush(&mut self, area: Rect, colors: &[Color]) -> Result<(), WaylandError> {
        let geometry = self.geometry.ok_or(WaylandError::NotConfigured)?;
        let valid = area.x >= 0
            && area.y >= 0
            && area.width > 0
            && area.height > 0
            && area.x.saturating_add(area.width)
                <= i32::try_from(geometry.logical_width)
                    .map_err(|_| WaylandError::GeometryOverflow)?
            && area.y.saturating_add(area.height)
                <= i32::try_from(geometry.logical_height)
                    .map_err(|_| WaylandError::GeometryOverflow)?;
        if !valid {
            return Err(WaylandError::InvalidDamage(area));
        }
        let area_width = usize::try_from(area.width).map_err(|_| WaylandError::GeometryOverflow)?;
        let area_height =
            usize::try_from(area.height).map_err(|_| WaylandError::GeometryOverflow)?;
        let expected = area_width
            .checked_mul(area_height)
            .ok_or(WaylandError::GeometryOverflow)?;
        if colors.len() != expected {
            return Err(WaylandError::PixelLength {
                expected,
                actual: colors.len(),
            });
        }

        let shadow_stride =
            usize::try_from(geometry.logical_width).map_err(|_| WaylandError::GeometryOverflow)?;
        let x = usize::try_from(area.x).map_err(|_| WaylandError::GeometryOverflow)?;
        let y = usize::try_from(area.y).map_err(|_| WaylandError::GeometryOverflow)?;
        for row in 0..area_height {
            let source_start = row * area_width;
            let destination_start = (y + row) * shadow_stride + x;
            self.shadow[destination_start..destination_start + area_width]
                .copy_from_slice(&colors[source_start..source_start + area_width]);
        }
        self.damage.add(area);
        Ok(())
    }

    fn request_present(&mut self) {
        self.submission.request_present();
    }

    fn try_submit(&mut self) -> Result<(), WaylandError> {
        if !self.submission.can_probe_slot() {
            return Ok(());
        }
        let geometry = match self.geometry {
            Some(geometry) => geometry,
            None => return Ok(()),
        };
        let generation = match self.active.as_mut() {
            Some(generation) => generation,
            None => return Ok(()),
        };
        let Some(slot_index) = generation.write_free_slot(&self.shadow)? else {
            return Ok(());
        };

        let surface = self.window.wl_surface();
        if self.damage.full || self.damage.rects.is_empty() {
            let (x, y, width, height) = geometry.full_buffer_damage();
            surface.damage_buffer(x, y, width, height);
        } else {
            for rect in &self.damage.rects {
                if let Some((x, y, width, height)) = geometry.map_damage(*rect) {
                    surface.damage_buffer(x, y, width, height);
                }
            }
        }
        surface.frame(&self.qh, FrameCallbackData(surface.clone()));
        generation.attach(slot_index, surface)?;
        self.window.commit();

        self.submission.submitted();
        self.damage.clear();
        self.submitted_frames = self.submitted_frames.saturating_add(1);
        Ok(())
    }

    fn retire_active(&mut self) {
        if let Some(mut generation) = self.active.take()
            && !generation.all_free()
        {
            self.retired.push(generation);
        }
    }

    fn release_retired(&mut self) {
        let mut index = 0;
        while index < self.retired.len() {
            if self.retired[index].all_free() {
                self.retired.swap_remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn retired_pool_bytes(&self) -> Result<usize, WaylandError> {
        self.retired.iter().try_fold(0usize, |total, generation| {
            total
                .checked_add(generation.pool_bytes())
                .ok_or(WaylandError::GeometryOverflow)
        })
    }

    fn stats(&mut self) -> WaylandDisplayStats {
        self.release_retired();
        let generation_bytes = self
            .active
            .as_ref()
            .map_or(0, ShmGeneration::pool_bytes)
            .saturating_add(
                self.retired
                    .iter()
                    .map(ShmGeneration::pool_bytes)
                    .sum::<usize>(),
            );
        WaylandDisplayStats {
            submitted_frames: self.submitted_frames,
            frame_callbacks: self.frame_callbacks,
            coalesced_presents: self.submission.coalesced_presents(),
            allocated_bytes: generation_bytes
                .saturating_add(self.shadow.len().saturating_mul(size_of::<Color>())),
            retired_generations: self.retired.len(),
        }
    }
}
