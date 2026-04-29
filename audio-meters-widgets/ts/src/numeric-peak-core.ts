// Headless core for NumericPeak. Mirror of widgets/src/meters/numeric.rs.
// Decouples canvas access via NumericSink so unit tests can record
// fillRect + drawText calls under Node without DOM globals.

import {
  BallisticState,
  type Ballistic,
} from "../../../audio-meters-core/ts/src/index.ts";
import { dbfsToScaleUnits, type Scale, type Skin, zoneColorFor } from "./skin.ts";

const DEFAULT_TEXT = "#dde2e6";
const DEFAULT_BACKGROUND = "#08080a";
const PEAK_DECAY_DB_PER_S = 12.0;

/** Drawing surface. Element binds to canvas; tests record. */
export interface NumericSink {
  fillRect(x: number, y: number, w: number, h: number, color: string): void;
  drawText(x: number, y: number, text: string, color: string): void;
}

export interface NumericPeakConfig {
  scale: Scale;
  skin: Skin;
  ballistic?: Ballistic;
}

export class NumericPeakCore {
  readonly scale: Scale;
  readonly skin: Skin;
  private state: BallisticState;
  private readingDb: number;
  private peakDb: number;
  private peakAgeS: number;
  private readonly NEG_FLOOR = -120.0;

  constructor(cfg: NumericPeakConfig) {
    if (cfg.skin.meter_type !== "numeric") {
      throw new Error(
        `NumericPeakCore: skin '${cfg.skin.id}' has meter_type '${cfg.skin.meter_type}', expected 'numeric'`,
      );
    }
    this.scale = cfg.scale;
    this.skin = cfg.skin;
    this.state = new BallisticState(
      (cfg.ballistic ?? cfg.skin.default_ballistic) as Ballistic,
    );
    this.readingDb = this.NEG_FLOOR;
    this.peakDb = this.NEG_FLOOR;
    this.peakAgeS = 0;
  }

  setBallistic(kind: Ballistic): void {
    this.state = new BallisticState(kind);
    this.readingDb = this.NEG_FLOOR;
    this.peakDb = this.NEG_FLOOR;
    this.peakAgeS = 0;
  }

  reset(): void {
    this.state.reset();
    this.readingDb = this.NEG_FLOOR;
    this.peakDb = this.NEG_FLOOR;
    this.peakAgeS = 0;
  }

  update(dbfs: number, dt: number): number {
    this.readingDb = this.state.update(dbfs, dt);
    if (this.readingDb >= this.peakDb) {
      this.peakDb = this.readingDb;
      this.peakAgeS = 0;
    } else {
      this.peakAgeS += dt;
      const holdS = (this.skin.layout.peak_hold_ms ?? 0) / 1000;
      if (this.peakAgeS > holdS) {
        const overage = this.peakAgeS - holdS;
        this.peakDb -= PEAK_DECAY_DB_PER_S * overage;
        this.peakAgeS = holdS;
        if (this.peakDb < this.readingDb) {
          this.peakDb = this.readingDb;
          this.peakAgeS = 0;
        }
      }
    }
    return this.readingDb;
  }

  readingDbValue(): number {
    return this.readingDb;
  }

  peakDbValue(): number {
    return this.peakDb;
  }

  /** Paint into `sink`. Two text lines: reading, peak hold. */
  draw(sink: NumericSink, x: number, y: number, w: number, h: number): void {
    const sec = this.skin.secondary_colors ?? {};
    const bg = sec.background ?? DEFAULT_BACKGROUND;
    const textDefault = sec.scale_text ?? DEFAULT_TEXT;

    sink.fillRect(x, y, w, h, bg);

    const scale = this.scale;
    const readingSu = dbfsToScaleUnits(scale, this.readingDb);
    const peakSu = dbfsToScaleUnits(scale, this.peakDb);

    const readingCol =
      readingSu <= scale.range_db.min
        ? textDefault
        : this.skin.palette[zoneColorFor(scale, readingSu)];
    const peakCol =
      peakSu <= scale.range_db.min
        ? textDefault
        : sec.peak_hold ?? this.skin.palette[zoneColorFor(scale, peakSu)];

    const units = scale.label_units;
    const readingText = `${formatPaddedNumber(readingSu, 7, 1)} ${units}`;
    const peakText = `PK ${formatPaddedNumber(peakSu, 6, 1)} ${units}`;

    const pad = 6;
    const topX = x + pad;
    const topY = y + Math.floor(h / 2) - 2;
    const botX = x + pad;
    const botY = y + h - pad;
    sink.drawText(topX, topY, readingText, readingCol);
    sink.drawText(botX, botY, peakText, peakCol);
  }
}

/**
 * Right-pad a number to `width` characters with `decimals` after the
 * point. Mirrors Rust's `{:>7.1}` formatting so the two runtimes
 * produce identical text on the same input.
 */
function formatPaddedNumber(
  value: number,
  width: number,
  decimals: number,
): string {
  const s = value.toFixed(decimals);
  if (s.length >= width) return s;
  return " ".repeat(width - s.length) + s;
}
