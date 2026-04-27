// Vanilla-canvas custom-element widgets for rlvgl audio meters.
// Browser-only; consumes @rlvgl/audio-meters-core for ballistic state.

export {
  LedBargraphCore,
  type DrawSink,
  type LedBargraphConfig,
} from "./led-bargraph-core.ts";
export {
  RlvglLedBargraphElement,
  defineRlvglLedBargraph,
} from "./led-bargraph-element.ts";
export {
  NeedleVuCore,
  NEEDLE_HALF_ARC_RAD,
  type NeedleSink,
  type NeedleVuConfig,
} from "./needle-vu-core.ts";
export {
  RlvglNeedleVuElement,
  defineRlvglNeedleVu,
} from "./needle-vu-element.ts";
export {
  NumericPeakCore,
  type NumericSink,
  type NumericPeakConfig,
} from "./numeric-peak-core.ts";
export {
  RlvglNumericPeakElement,
  defineRlvglNumericPeak,
} from "./numeric-peak-element.ts";
export {
  type Layout,
  type MeterColorId,
  type MeterType,
  type Orientation,
  type Palette,
  type Scale,
  type SecondaryColors,
  type Skin,
  type Zone,
  dbfsToScaleUnits,
  zoneColorFor,
} from "./skin.ts";
