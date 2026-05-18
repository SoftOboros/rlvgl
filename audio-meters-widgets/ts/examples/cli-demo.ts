// CLI demo for the rlvgl audio-meters TS widgets. Drives all four
// widget cores with a synthetic dBFS sequence and prints per-frame
// readings to stdout. Run with:
//
//   node --experimental-strip-types --no-warnings examples/cli-demo.ts
// or:
//   npm run demo
//
// Output:
//
//   t   |  Bar  |   Needle   |   Numeric    | LUFS gauge
//   0.0 |   off |    -50.0°  |  -120.0 dBFS | I=-23.0 S=-23.0 M=-23.0
//   ...

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { LedBargraphCore } from "../src/led-bargraph-core.ts";
import { NeedleVuCore } from "../src/needle-vu-core.ts";
import { NumericPeakCore } from "../src/numeric-peak-core.ts";
import { LufsGaugeCore } from "../src/lufs-gauge-core.ts";
import type { Scale, Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

function loadJson<T>(rel: string): T {
  return JSON.parse(readFileSync(join(ASSETS, rel), "utf8")) as T;
}

const VU = loadJson<Scale>("scales/vu_broadcast.json");
const VU_BAR = loadJson<Skin>("skins/broadcast_classic_bargraph.json");
const VU_NEEDLE = loadJson<Skin>("skins/broadcast_classic_needle.json");
const DIGITAL = loadJson<Scale>("scales/digital_peak.json");
const NUMERIC = loadJson<Skin>("skins/digital_studio_numeric.json");
const LUFS = loadJson<Scale>("scales/lufs_ebu_r128.json");
const LUFS_SKIN = loadJson<Skin>("skins/lufs_ebu_r128_gauge.json");

const bar = new LedBargraphCore({
  scale: VU,
  skin: VU_BAR,
  showTicks: false,
});
const needle = new NeedleVuCore({ scale: VU, skin: VU_NEEDLE });
const numeric = new NumericPeakCore({ scale: DIGITAL, skin: NUMERIC });
const lufs = new LufsGaugeCore({ scale: LUFS, skin: LUFS_SKIN });

/**
 * Synthetic dBFS sequence over 12 seconds:
 *
 *   0..2  : silent
 *   2..4  : ramp -60 → -10
 *   4..7  : steady -10
 *   7..9  : transient at -1, otherwise -10
 *   9..12 : silent
 */
function synthetic(t: number): number {
  if (t < 2) return -120;
  if (t < 4) return -60 + ((t - 2) / 2) * 50;
  if (t < 7) return -10;
  if (t < 9) {
    // Pulse train: -1 dBFS for 100 ms, -10 dBFS otherwise.
    const cycle = (t - 7) % 0.5;
    return cycle < 0.1 ? -1 : -10;
  }
  return -120;
}

/** ASCII bargraph: 16 cells. */
function asciiBar(dbfs: number, scale: Scale): string {
  const sv = dbfs + (scale.pivot.value - scale.pivot.input_dbfs);
  const lo = scale.range_db.min;
  const hi = scale.range_db.max;
  const t = Math.max(0, Math.min(1, (sv - lo) / (hi - lo)));
  const N = 16;
  const lit = Math.round(t * N);
  return "▕" + "▆".repeat(lit) + "·".repeat(N - lit) + "▏";
}

const dt = 1 / 60;
const totalFrames = 12 * 60;
const printEvery = 30;

console.log("== rlvgl audio meters CLI demo ==");
console.log(
  "  t   | Bargraph (VU)        | Needle (VU) | Numeric (dBFS)  | LUFS gauge",
);
console.log(
  "------+----------------------+-------------+-----------------+---------------------",
);

for (let f = 0; f < totalFrames; f++) {
  const t = f * dt;
  const dbfs = synthetic(t);
  bar.update(dbfs, dt);
  needle.update(dbfs, dt);
  numeric.update(dbfs, dt);
  lufs.update(dbfs, dt);

  if (f % printEvery === 0) {
    const tStr = t.toFixed(1).padStart(4, " ");
    const barStr = asciiBar(bar.readingDbValue(), VU);
    const angDeg = (
      (needle.needleAngleRad() * 180) /
      Math.PI
    ).toFixed(0);
    const needleStr = `${angDeg.padStart(4, " ")}°`;
    const numStr =
      `${numeric.readingDbValue().toFixed(1).padStart(7, " ")}` +
      ` PK ${numeric.peakDbValue().toFixed(1).padStart(7, " ")}`;
    const lufsStr =
      `I=${lufs.integratedDbValue().toFixed(1).padStart(6, " ")}` +
      ` S=${lufs.shortTermDbValue().toFixed(1).padStart(6, " ")}` +
      ` M=${lufs.momentaryDbValue().toFixed(1).padStart(6, " ")}`;
    console.log(
      `${tStr}  | ${barStr} | ${needleStr.padEnd(11, " ")} | ${numStr} | ${lufsStr}`,
    );
  }
}

console.log("\nDone — replace `synthetic()` with real dBFS from your audio source.");
