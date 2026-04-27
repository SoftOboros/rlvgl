// Ballistic state machines.
// Mirror of audio-meters-core/src/ballistic.rs. Mathematics MUST match the
// Rust reference to within the parity tolerance defined by the shared
// fixtures (currently 1e-4 dB). When in doubt, change Rust first; this
// port follows.
//
// All internal arithmetic is rounded to f32 via `Math.fround` to match the
// Rust f32 single-precision pipeline. Without this, leaky-integrator
// accumulation drifts measurably across hundreds of frames.

const fr = Math.fround;

/** Lower clamp for ballistic state, dB-domain. See concepts §5. */
export const NEG_INFINITY_FLOOR_DB: number = -120.0;

const NEG_INFINITY_FLOOR_AMP: number = fr(1.0e-6); // 10^(-120/20)

// ---- VU ---------------------------------------------------------------

const VU_TAU_S = fr(0.0651);

// ---- PPM Type I (DIN 45406) ------------------------------------------

const PPM_I_TAU_ATTACK_S = fr(0.00226);
const PPM_I_DECAY_DB_PER_S = fr(20.0 / 1.5);

// ---- PPM Type IIa (BBC) ----------------------------------------------

const PPM_IIA_TAU_ATTACK_S = fr(0.00452);
const PPM_IIA_DECAY_DB_PER_S = fr(24.0 / 2.8);

// ---- PPM Type IIb (EBU) ----------------------------------------------

const PPM_IIB_TAU_ATTACK_S = fr(0.00452);
const PPM_IIB_DECAY_DB_PER_S = fr(20.0 / 1.7);

// ---- Digital peak ----------------------------------------------------

const DIGITAL_PEAK_DECAY_DB_PER_S = fr(20.0 / 1.5);

/**
 * Absolute-gate threshold for `LufsI`. Per ITU-R BS.1770-4, blocks
 * below -70 LUFS are excluded from the integrated mean. We apply
 * per-sample (streaming) since L0 is not block-based; relative
 * gating (programme-mean − 10 LU) is deferred. See concepts §15-006.
 */
const LUFS_ABSOLUTE_GATE_DB = -70.0;

// ---- Windowed (RMS / LUFS) -------------------------------------------

const RMS_LUFS_M_TAU_S = fr(0.100);
const LUFS_S_TAU_S = fr(0.750);

/** Frozen variant set. Concepts §5; registration policy: Standards Action. */
export type Ballistic =
  | "Vu"
  | "PpmTypeI"
  | "PpmTypeIIa"
  | "PpmTypeIIb"
  | "DigitalPeak"
  | "Rms"
  | "LufsM"
  | "LufsS"
  | "LufsI"
  | "Instant";

/** All variants, declaration-ordered. Useful for parity tests. */
export const ALL_BALLISTICS: readonly Ballistic[] = [
  "Vu",
  "PpmTypeI",
  "PpmTypeIIa",
  "PpmTypeIIb",
  "DigitalPeak",
  "Rms",
  "LufsM",
  "LufsS",
  "LufsI",
  "Instant",
];

interface State {
  kind: Ballistic;
  readingDb: number;
  linState: number;
  integratedCount: number;
  integratedMean: number;
}

/** Per-meter state. Construct with `newState`, advance with `update`. */
export class BallisticState {
  private s: State;

  constructor(kind: Ballistic) {
    this.s = {
      kind,
      readingDb: NEG_INFINITY_FLOOR_DB,
      linState: 0.0,
      integratedCount: 0,
      integratedMean: 0.0,
    };
  }

  reset(): void {
    this.s.readingDb = NEG_INFINITY_FLOOR_DB;
    this.s.linState = 0.0;
    this.s.integratedCount = 0;
    this.s.integratedMean = 0.0;
  }

  readingDb(): number {
    return this.s.readingDb;
  }

  kind(): Ballistic {
    return this.s.kind;
  }

  /**
   * Advance the ballistic by one frame. `dbfs` is the per-frame input;
   * `dt` is the frame interval in seconds. Returns the new reading in
   * dBFS.
   */
  update(dbfs: number, dt: number): number {
    const dbfsClean = sanitiseDbfs(dbfs);
    const dtClean = Number.isFinite(dt) && dt > 0.0 ? fr(dt) : 0.0;

    switch (this.s.kind) {
      case "Instant":
        // §5: zero ballistic, identity.
        this.s.readingDb = fr(dbfsClean);
        break;
      case "Vu": {
        // §5: IEC 60268-17, symmetric linear-amplitude lowpass.
        this.s.linState = stepAmpLowpass(
          this.s.linState,
          dbfsClean,
          dtClean,
          VU_TAU_S,
        );
        this.s.readingDb = ampToDb(this.s.linState);
        break;
      }
      case "PpmTypeI":
        this.s.linState = stepPpmAmp(
          this.s.linState,
          dbfsClean,
          dtClean,
          PPM_I_TAU_ATTACK_S,
          PPM_I_DECAY_DB_PER_S,
        );
        this.s.readingDb = ampToDb(this.s.linState);
        break;
      case "PpmTypeIIa":
        this.s.linState = stepPpmAmp(
          this.s.linState,
          dbfsClean,
          dtClean,
          PPM_IIA_TAU_ATTACK_S,
          PPM_IIA_DECAY_DB_PER_S,
        );
        this.s.readingDb = ampToDb(this.s.linState);
        break;
      case "PpmTypeIIb":
        this.s.linState = stepPpmAmp(
          this.s.linState,
          dbfsClean,
          dtClean,
          PPM_IIB_TAU_ATTACK_S,
          PPM_IIB_DECAY_DB_PER_S,
        );
        this.s.readingDb = ampToDb(this.s.linState);
        break;
      case "DigitalPeak":
        // §5: instantaneous attack, PPM-I-style decay (dB-domain).
        if (dbfsClean >= this.s.readingDb) {
          this.s.readingDb = fr(dbfsClean);
        } else {
          this.s.readingDb = fr(
            this.s.readingDb - DIGITAL_PEAK_DECAY_DB_PER_S * dtClean,
          );
          if (this.s.readingDb < NEG_INFINITY_FLOOR_DB) {
            this.s.readingDb = NEG_INFINITY_FLOOR_DB;
          }
        }
        break;
      case "Rms":
      case "LufsM":
        this.s.linState = stepPowerLeaky(
          this.s.linState,
          dbfsClean,
          dtClean,
          RMS_LUFS_M_TAU_S,
        );
        this.s.readingDb = powerToDb(this.s.linState);
        break;
      case "LufsS":
        this.s.linState = stepPowerLeaky(
          this.s.linState,
          dbfsClean,
          dtClean,
          LUFS_S_TAU_S,
        );
        this.s.readingDb = powerToDb(this.s.linState);
        break;
      case "LufsI": {
        // §5: BS.1770 absolute-gated running mean. Skip samples below
        // LUFS_ABSOLUTE_GATE_DB; hold previous reading during silence.
        if (dbfsClean >= LUFS_ABSOLUTE_GATE_DB) {
          const p = dbToPower(dbfsClean);
          if (this.s.integratedCount < 0xffffffff) {
            this.s.integratedCount += 1;
          }
          const n = fr(this.s.integratedCount);
          this.s.integratedMean = fr(
            this.s.integratedMean + (p - this.s.integratedMean) / n,
          );
          this.s.readingDb = powerToDb(this.s.integratedMean);
        }
        break;
      }
    }

    if (this.s.readingDb < NEG_INFINITY_FLOOR_DB) {
      this.s.readingDb = NEG_INFINITY_FLOOR_DB;
    }
    return this.s.readingDb;
  }
}

function stepAmpLowpass(
  amp: number,
  dbfs: number,
  dt: number,
  tauS: number,
): number {
  const aIn = dbToAmp(dbfs);
  const alpha = oneMinusExp(fr(-dt / tauS));
  return fr(amp + (aIn - amp) * alpha);
}

function stepPpmAmp(
  amp: number,
  dbfs: number,
  dt: number,
  tauAttackS: number,
  decayDbPerS: number,
): number {
  const aIn = dbToAmp(dbfs);
  let next: number;
  if (aIn > amp) {
    const alpha = oneMinusExp(fr(-dt / tauAttackS));
    next = fr(amp + (aIn - amp) * alpha);
  } else {
    // Linear-dB decay → multiplicative in linear amplitude.
    const factor = exp10(fr((-decayDbPerS * dt) / 20.0));
    next = fr(amp * factor);
    if (next < NEG_INFINITY_FLOOR_AMP) {
      next = 0.0;
    }
  }
  return next;
}

function stepPowerLeaky(
  power: number,
  dbfs: number,
  dt: number,
  tauS: number,
): number {
  const pIn = dbToPower(dbfs);
  const alpha = oneMinusExp(fr(-dt / tauS));
  return fr(power + (pIn - power) * alpha);
}

function oneMinusExp(x: number): number {
  if (x >= 0) return 0.0;
  return fr(1.0 - Math.exp(x));
}

function dbToAmp(db: number): number {
  return fr(Math.pow(10, db / 20.0));
}

function dbToPower(db: number): number {
  return fr(Math.pow(10, db / 10.0));
}

function exp10(x: number): number {
  return fr(Math.pow(10, x));
}

function ampToDb(a: number): number {
  if (a <= NEG_INFINITY_FLOOR_AMP) return NEG_INFINITY_FLOOR_DB;
  return fr(20.0 * Math.log10(a));
}

function powerToDb(p: number): number {
  if (p <= 0.0) return NEG_INFINITY_FLOOR_DB;
  return fr(10.0 * Math.log10(p));
}

function sanitiseDbfs(dbfs: number): number {
  if (!Number.isFinite(dbfs)) return NEG_INFINITY_FLOOR_DB;
  if (dbfs < NEG_INFINITY_FLOOR_DB) return NEG_INFINITY_FLOOR_DB;
  return dbfs;
}
