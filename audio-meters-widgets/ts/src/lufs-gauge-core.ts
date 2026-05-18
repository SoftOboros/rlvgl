// Headless core for LufsGauge. Mirror of widgets/src/meters/lufs_gauge.rs.
// Compound widget: owns three BallisticState instances (LufsM,
// LufsS, LufsI), drives them from a single `update(dbfs, dt)` call,
// renders three numeric lines with the integrated reading colour-
// coded against target.

import {
  BallisticState,
} from "../../../audio-meters-core/ts/src/index.ts";
import { dbfsToScaleUnits, type Scale, type Skin } from "./skin.ts";

const DEFAULT_TEXT = "#dde2e6";
const DEFAULT_BACKGROUND = "#08080a";
const NEG_FLOOR = -120;
const NOMINAL_LU_HALF_WIDTH = 0.5;
const CAUTION_LU_HALF_WIDTH = 1.5;

export interface LufsSink {
  fillRect(x: number, y: number, w: number, h: number, color: string): void;
  drawText(x: number, y: number, text: string, color: string): void;
}

export interface LufsGaugeConfig {
  scale: Scale;
  skin: Skin;
}

export class LufsGaugeCore {
  readonly scale: Scale;
  readonly skin: Skin;
  private momentary: BallisticState;
  private shortTerm: BallisticState;
  private integrated: BallisticState;
  private lastM: number = NEG_FLOOR;
  private lastS: number = NEG_FLOOR;
  private lastI: number = NEG_FLOOR;

  constructor(cfg: LufsGaugeConfig) {
    if (cfg.skin.meter_type !== "lufs_gauge") {
      throw new Error(
        `LufsGaugeCore: skin '${cfg.skin.id}' has meter_type '${cfg.skin.meter_type}', expected 'lufs_gauge'`,
      );
    }
    this.scale = cfg.scale;
    this.skin = cfg.skin;
    this.momentary = new BallisticState("LufsM");
    this.shortTerm = new BallisticState("LufsS");
    this.integrated = new BallisticState("LufsI");
  }

  reset(): void {
    this.momentary.reset();
    this.shortTerm.reset();
    this.integrated.reset();
    this.lastM = NEG_FLOOR;
    this.lastS = NEG_FLOOR;
    this.lastI = NEG_FLOOR;
  }

  update(dbfs: number, dt: number): void {
    this.lastM = this.momentary.update(dbfs, dt);
    this.lastS = this.shortTerm.update(dbfs, dt);
    this.lastI = this.integrated.update(dbfs, dt);
  }

  momentaryDbValue(): number {
    return this.lastM;
  }
  shortTermDbValue(): number {
    return this.lastS;
  }
  integratedDbValue(): number {
    return this.lastI;
  }

  /** Target LUFS — the bound scale's `pivot.value`. */
  targetLufs(): number {
    return this.scale.pivot.value;
  }

  private integratedColor(): string {
    if (this.lastI <= this.scale.range_db.min) {
      return this.skin.secondary_colors?.scale_text ?? DEFAULT_TEXT;
    }
    const lu =
      dbfsToScaleUnits(this.scale, this.lastI) - this.targetLufs();
    const abs = Math.abs(lu);
    if (abs <= NOMINAL_LU_HALF_WIDTH) return this.skin.palette.Nominal;
    if (abs <= CAUTION_LU_HALF_WIDTH) return this.skin.palette.Caution;
    return lu > 0 ? this.skin.palette.Hot : this.skin.palette.Safe;
  }

  draw(sink: LufsSink, x: number, y: number, w: number, h: number): void {
    const sec = this.skin.secondary_colors ?? {};
    const bg = sec.background ?? DEFAULT_BACKGROUND;
    const textDefault = sec.scale_text ?? DEFAULT_TEXT;

    sink.fillRect(x, y, w, h, bg);

    const units = this.scale.label_units;
    const iLufs = dbfsToScaleUnits(this.scale, this.lastI);
    const sLufs = dbfsToScaleUnits(this.scale, this.lastS);
    const mLufs = dbfsToScaleUnits(this.scale, this.lastM);
    const iLu = iLufs - this.targetLufs();

    const pad = 6;
    const line1Y = y + Math.floor(h / 3) - 4;
    const line2Y = y + Math.floor((2 * h) / 3) - 4;
    const line3Y = y + h - pad;
    const lineX = x + pad;

    const fmt = (v: number, width: number) => {
      const s = v.toFixed(1);
      return s.length >= width ? s : " ".repeat(width - s.length) + s;
    };
    const luSign = iLu >= 0 ? "+" : "";

    sink.drawText(
      lineX,
      line1Y,
      `I  ${fmt(iLufs, 6)} ${units}  (${luSign}${iLu.toFixed(1)} LU)`,
      this.integratedColor(),
    );
    sink.drawText(
      lineX,
      line2Y,
      `S  ${fmt(sLufs, 6)} ${units}`,
      textDefault,
    );
    sink.drawText(
      lineX,
      line3Y,
      `M  ${fmt(mLufs, 6)} ${units}`,
      textDefault,
    );
  }
}
