// Pure rendering core for LedBargraph. Decoupled from DOM so it is
// testable under Node by passing in a stand-in 2D context. The custom
// element in led-bargraph-element.ts wraps this with `<canvas>` glue.

// Until the workspace publishes packages, import the L0 package from
// its source location. When publishing, switch to "@rlvgl/audio-meters-core".
import {
  BallisticState,
  type Ballistic,
} from "../../../audio-meters-core/ts/src/index.ts";
import { type Scale, type Skin, zoneColorFor } from "./skin.ts";

const DEFAULT_LED_OFF = "#141418";
const DEFAULT_BACKGROUND = "#08080a";
const DEFAULT_PEAK_HOLD = "#ffffff";

/** Decay rate for the peak-hold pip after the dwell expires (dB / s). */
const PEAK_DECAY_DB_PER_S = 12.0;

/** A "draw rectangle" call. The element binds this to canvas 2D context. */
export interface DrawSink {
  fillRect(x: number, y: number, w: number, h: number, color: string): void;
}

export interface LedBargraphConfig {
  scale: Scale;
  skin: Skin;
  /** Override the skin's default ballistic. */
  ballistic?: Ballistic;
}

export class LedBargraphCore {
  readonly scale: Scale;
  readonly skin: Skin;
  private state: BallisticState;
  private readingDb: number;
  private peakDb: number;
  private peakAgeS: number;
  private readonly NEG_FLOOR = -120.0;

  constructor(cfg: LedBargraphConfig) {
    if (cfg.skin.meter_type !== "bargraph") {
      throw new Error(
        `LedBargraphCore: skin '${cfg.skin.id}' has meter_type '${cfg.skin.meter_type}', expected 'bargraph'`,
      );
    }
    this.scale = cfg.scale;
    this.skin = cfg.skin;
    const kind = (cfg.ballistic ?? cfg.skin.default_ballistic) as Ballistic;
    this.state = new BallisticState(kind);
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

  /** Advance one frame. `dt` in seconds, `dbfs` in dBFS. */
  update(dbfs: number, dt: number): void {
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
  }

  readingDbValue(): number {
    return this.readingDb;
  }

  peakDbValue(): number {
    return this.peakDb;
  }

  /**
   * Paint into `sink` over a rectangle of size `w x h` (top-left at
   * `x,y`). Same draw model as widgets::meters::bargraph::LedBargraph::draw
   * on the Rust side: 1 background fill + N segment fills + optional
   * peak pip.
   */
  draw(sink: DrawSink, x: number, y: number, w: number, h: number): void {
    const skin = this.skin;
    const scale = this.scale;
    const sec = skin.secondary_colors ?? {};
    const bg = sec.background ?? DEFAULT_BACKGROUND;
    const off = sec.led_off ?? DEFAULT_LED_OFF;
    const peakColor = sec.peak_hold ?? DEFAULT_PEAK_HOLD;
    const n = Math.max(1, skin.layout.led_count ?? 1);

    sink.fillRect(x, y, w, h, bg);

    const cal = scale.calibration_default?.offset_db ?? 0;
    const readingDisp = this.readingDb + cal;
    const peakDisp = this.peakDb + cal;
    const lo = scale.range_db.min;
    const hi = scale.range_db.max;
    const span = Math.max(hi - lo, Number.EPSILON);

    const litFrac = clamp01((readingDisp - lo) / span);
    const litSegments = Math.round(litFrac * n);
    const peakFrac = clamp01((peakDisp - lo) / span);
    const peakSegment = clampInt(Math.round(peakFrac * n) - 1, 0, n - 1);

    const horizontal = skin.layout.orientation === "horizontal";
    const palette = skin.palette;

    for (let i = 0; i < n; i++) {
      const cell = segmentRect(x, y, w, h, n, i, horizontal);
      const centreFrac = (i + 0.5) / n;
      const centreDb = lo + centreFrac * span;
      const colorId = zoneColorFor(scale, centreDb);
      const zoneCol = palette[colorId];

      const lit = i < litSegments;
      sink.fillRect(cell.x, cell.y, cell.w, cell.h, lit ? zoneCol : off);

      if (
        i === peakSegment &&
        (skin.layout.peak_hold_ms ?? 0) > 0 &&
        peakDisp > lo &&
        litSegments < n
      ) {
        sink.fillRect(cell.x, cell.y, cell.w, cell.h, peakColor);
      }
    }
  }
}

interface Cell {
  x: number;
  y: number;
  w: number;
  h: number;
}

function segmentRect(
  x: number,
  y: number,
  w: number,
  h: number,
  n: number,
  i: number,
  horizontal: boolean,
): Cell {
  if (horizontal) {
    const cellW = Math.floor(w / n);
    const cx = x + i * cellW;
    const cw = i === n - 1 ? w - i * cellW : cellW;
    return { x: cx, y, w: Math.max(1, cw), h };
  }
  const cellH = Math.floor(h / n);
  // Segment 0 is the bottom; visually that's `y + h - cellH` for i=0.
  const cy = y + h - (i + 1) * cellH;
  const ch = i === n - 1 ? h - (n - 1) * cellH : cellH;
  return {
    x,
    y: Math.max(y, cy),
    w,
    h: Math.max(1, ch),
  };
}

function clamp01(v: number): number {
  if (v < 0) return 0;
  if (v > 1) return 1;
  return v;
}

function clampInt(v: number, lo: number, hi: number): number {
  if (v < lo) return lo;
  if (v > hi) return hi;
  return v;
}
