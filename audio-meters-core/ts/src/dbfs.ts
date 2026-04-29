// dBFS helper and display-time calibration offset.
// Mirror of audio-meters-core/src/dbfs.rs. See
// docs/audio-meters/00-concepts.md §9.

/**
 * Apply a calibration offset to a dBFS reading.
 *
 * `offsetDb` is added directly: `dbu = dbfs + offsetDb`. The offset is
 * per-installation (the studio chose `0 dBu = -20 dBFS`,
 * `0 dBu = -18 dBFS`, etc.). Skin descriptors carry a default; widgets may
 * override.
 */
export function applyCalibration(dbfs: number, offsetDb: number): number {
  return dbfs + offsetDb;
}
