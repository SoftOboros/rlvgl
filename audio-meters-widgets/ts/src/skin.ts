// Runtime Skin / Scale types for TS audio-meter widgets.
// Mirror of widgets/src/meters/skin.rs. Both runtimes load the JSON
// descriptors under assets/audio-meters/ and project them into these
// types. Field names use snake_case to match the JSON exactly so the
// loader does not have to remap.

/** Concepts §7 colour-zone identifier. */
export type MeterColorId =
  | "Safe"
  | "Nominal"
  | "Caution"
  | "Hot"
  | "Over";

/** Concrete §7 colours bound to identifiers. Hex strings, e.g. `#1f9d4f`. */
export interface Palette {
  Safe: string;
  Nominal: string;
  Caution: string;
  Hot: string;
  Over: string;
}

/** Optional non-zone colours; widget defaults apply when fields are absent. */
export interface SecondaryColors {
  background?: string;
  frame?: string;
  scale_text?: string;
  minor_tick?: string;
  major_tick?: string;
  needle?: string;
  needle_pivot?: string;
  led_off?: string;
  peak_hold?: string;
}

export type Orientation = "horizontal" | "vertical";
export type MeterType = "bargraph" | "needle" | "numeric" | "lufs_gauge";

export interface Layout {
  orientation: Orientation;
  aspect_ratio: number;
  led_count?: number;
  peak_hold_ms?: number;
}

export interface Zone {
  from_db: number;
  to_db: number;
  color: MeterColorId;
}

export interface Scale {
  id: string;
  label_units: string;
  range_db: { min: number; max: number };
  pivot: { label: string; input_dbfs: number };
  calibration_default?: { to: string; offset_db: number };
  ticks: {
    majors: number[];
    minors_per_major_division: number;
    labels?: Record<string, string>;
  };
  zones: Zone[];
  compatible_ballistics: string[];
}

export interface Skin {
  id: string;
  title: string;
  scale_id: string;
  default_ballistic: string;
  calibration_override?: { to: string; offset_db: number };
  meter_type: MeterType;
  palette: Palette;
  secondary_colors?: SecondaryColors;
  layout: Layout;
}

/** Look up the §7 colour identifier whose zone covers `dbValue`. */
export function zoneColorFor(scale: Scale, dbValue: number): MeterColorId {
  for (const z of scale.zones) {
    if (dbValue < z.to_db) return z.color;
  }
  return scale.zones[scale.zones.length - 1]?.color ?? "Safe";
}
