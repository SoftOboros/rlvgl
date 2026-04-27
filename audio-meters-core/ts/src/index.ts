// L0 ballistics + dB calibration helpers for rlvgl audio meters.
// Hand-ported from audio-meters-core (Rust). See
// docs/audio-meters/00-concepts.md.

export {
  BallisticState,
  ALL_BALLISTICS,
  NEG_INFINITY_FLOOR_DB,
} from "./ballistic.ts";
export type { Ballistic } from "./ballistic.ts";
export { applyCalibration } from "./dbfs.ts";
