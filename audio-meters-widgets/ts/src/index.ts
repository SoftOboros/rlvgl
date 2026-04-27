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
  type Layout,
  type MeterColorId,
  type MeterType,
  type Orientation,
  type Palette,
  type Scale,
  type SecondaryColors,
  type Skin,
  type Zone,
  zoneColorFor,
} from "./skin.ts";
