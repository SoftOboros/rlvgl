// SPDX-License-Identifier: MIT
//! Release-aware Wayland SHM slots.

use alloc::string::ToString;

use rlvgl_core::widget::Color;
use smithay_client_toolkit::shm::{
    Shm,
    slot::{Buffer, SlotPool},
};
use wayland_client::protocol::{wl_shm, wl_surface};

use super::{
    WaylandError,
    model::{Geometry, encode_complete_frame},
};

/// One exactly sized SCTK pool and its fixed presentation slots.
pub(crate) struct ShmGeneration {
    pool: SlotPool,
    buffers: alloc::vec::Vec<Buffer>,
    pub(crate) geometry: Geometry,
    pool_bytes: usize,
}

impl core::fmt::Debug for ShmGeneration {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ShmGeneration")
            .field("geometry", &self.geometry)
            .field("pool_bytes", &self.pool_bytes)
            .field("slot_count", &self.buffers.len())
            .finish()
    }
}

impl ShmGeneration {
    pub(crate) fn new(
        geometry: Geometry,
        slot_count: usize,
        shm: &Shm,
    ) -> Result<Self, WaylandError> {
        let pool_bytes = geometry
            .aligned_slot_bytes
            .checked_mul(slot_count)
            .ok_or(WaylandError::GeometryOverflow)?;
        i32::try_from(pool_bytes).map_err(|_| WaylandError::GeometryOverflow)?;
        let mut pool =
            SlotPool::new(pool_bytes, shm).map_err(|error| WaylandError::Shm(error.to_string()))?;
        let mut buffers = alloc::vec::Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            let buffer = {
                let (buffer, canvas) = pool
                    .create_buffer(
                        geometry.buffer_width,
                        geometry.buffer_height,
                        geometry.stride,
                        wl_shm::Format::Xrgb8888,
                    )
                    .map_err(|error| WaylandError::Shm(error.to_string()))?;
                canvas.fill(0);
                buffer
            };
            buffers.push(buffer);
        }
        if pool.len() != pool_bytes {
            return Err(WaylandError::AllocationInvariant {
                expected: pool_bytes,
                actual: pool.len(),
            });
        }
        Ok(Self {
            pool,
            buffers,
            geometry,
            pool_bytes,
        })
    }

    pub(crate) fn pool_bytes(&self) -> usize {
        self.pool_bytes
    }

    pub(crate) fn all_free(&mut self) -> bool {
        let Self { pool, buffers, .. } = self;
        buffers.iter().all(|buffer| buffer.canvas(pool).is_some())
    }

    pub(crate) fn write_free_slot(
        &mut self,
        shadow: &[Color],
    ) -> Result<Option<usize>, WaylandError> {
        let Self {
            pool,
            buffers,
            geometry,
            ..
        } = self;
        for (index, buffer) in buffers.iter().enumerate() {
            if let Some(canvas) = buffer.canvas(pool) {
                encode_complete_frame(*geometry, shadow, canvas)?;
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    pub(crate) fn attach(
        &self,
        index: usize,
        surface: &wl_surface::WlSurface,
    ) -> Result<(), WaylandError> {
        self.buffers
            .get(index)
            .ok_or(WaylandError::AllocationInvariant {
                expected: self.buffers.len(),
                actual: index,
            })?
            .attach_to(surface)
            .map_err(|error| WaylandError::Shm(error.to_string()))
    }
}
