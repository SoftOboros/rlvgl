// Headless core for NeedleVu. Mirror of widgets/src/meters/needle.rs.
// Decouples canvas access via a NeedleSink so unit tests can inspect
// what would be drawn under Node without a DOM.

import {
  BallisticState,
  type Ballistic,
} from "../../../audio-meters-core/ts/src/index.ts";
import { dbfsToScaleUnits, type Scale, type Skin } from "./skin.ts";

const DEFAULT_BACKGROUND = "#f1e7c4";
const DEFAULT_NEEDLE = "#1a1a1a";
const DEFAULT_PIVOT = "#3a2a18";

/** Half-arc angle, radians. Matches widgets::meters::needle Rust constant. */
export const NEEDLE_HALF_ARC_RAD = 0.8726646;
const NEEDLE_THICKNESS_PX = 2;
const PIVOT_RADIUS_PX = 4;

/**
 * Drawing surface. The element binds this to canvas 2D context;
 * tests provide a recorder.
 */
export interface NeedleSink {
  fillRect(x: number, y: number, w: number, h: number, color: string): void;
}

export interface NeedleVuConfig {
  scale: Scale;
  skin: Skin;
  ballistic?: Ballistic;
}

export class NeedleVuCore {
  readonly scale: Scale;
  readonly skin: Skin;
  private state: BallisticState;
  private readingDb: number;
  private readonly NEG_FLOOR = -120.0;

  constructor(cfg: NeedleVuConfig) {
    if (cfg.skin.meter_type !== "needle") {
      throw new Error(
        `NeedleVuCore: skin '${cfg.skin.id}' has meter_type '${cfg.skin.meter_type}', expected 'needle'`,
      );
    }
    this.scale = cfg.scale;
    this.skin = cfg.skin;
    this.state = new BallisticState(
      (cfg.ballistic ?? cfg.skin.default_ballistic) as Ballistic,
    );
    this.readingDb = this.NEG_FLOOR;
  }

  setBallistic(kind: Ballistic): void {
    this.state = new BallisticState(kind);
    this.readingDb = this.NEG_FLOOR;
  }

  reset(): void {
    this.state.reset();
    this.readingDb = this.NEG_FLOOR;
  }

  /** Advance one frame. */
  update(dbfs: number, dt: number): number {
    this.readingDb = this.state.update(dbfs, dt);
    return this.readingDb;
  }

  readingDbValue(): number {
    return this.readingDb;
  }

  /**
   * Needle angle in radians measured from straight-up (0 = vertical),
   * positive rightward. Mirrors Rust `NeedleVu::needle_angle_rad`.
   */
  needleAngleRad(): number {
    const sv = dbfsToScaleUnits(this.scale, this.readingDb);
    const lo = this.scale.range_db.min;
    const hi = this.scale.range_db.max;
    const span = Math.max(hi - lo, Number.EPSILON);
    const t = clamp01((sv - lo) / span);
    return -NEEDLE_HALF_ARC_RAD + t * 2 * NEEDLE_HALF_ARC_RAD;
  }

  /** Paint into `sink`. */
  draw(sink: NeedleSink, x: number, y: number, w: number, h: number): void {
    const sec = this.skin.secondary_colors ?? {};
    const bg = sec.background ?? DEFAULT_BACKGROUND;
    const needleColor = sec.needle ?? DEFAULT_NEEDLE;
    const pivotColor = sec.needle_pivot ?? DEFAULT_PIVOT;

    sink.fillRect(x, y, w, h, bg);

    const pivotX = x + Math.floor(w / 2);
    const pivotY = y + h - 1;
    const length = Math.floor(h * 0.95);
    const angle = this.needleAngleRad();

    drawNeedleLine(sink, pivotX, pivotY, length, angle, needleColor);
    drawPivotDot(sink, pivotX, pivotY, pivotColor);
  }
}

function drawNeedleLine(
  sink: NeedleSink,
  pivotX: number,
  pivotY: number,
  length: number,
  angle: number,
  color: string,
): void {
  const dx = Math.sin(angle);
  const dy = -Math.cos(angle);
  const half = NEEDLE_THICKNESS_PX >> 1;
  for (let s = 0; s <= length; s++) {
    const x = pivotX + s * dx;
    const y = pivotY + s * dy;
    sink.fillRect(
      Math.floor(x) - half,
      Math.floor(y) - half,
      NEEDLE_THICKNESS_PX,
      NEEDLE_THICKNESS_PX,
      color,
    );
  }
}

function drawPivotDot(
  sink: NeedleSink,
  cx: number,
  cy: number,
  color: string,
): void {
  sink.fillRect(
    cx - PIVOT_RADIUS_PX,
    cy - PIVOT_RADIUS_PX,
    PIVOT_RADIUS_PX * 2,
    PIVOT_RADIUS_PX * 2,
    color,
  );
}

function clamp01(v: number): number {
  if (v < 0) return 0;
  if (v > 1) return 1;
  return v;
}
