// Headless tests for NumericPeakCore. Mirror of the Rust-side tests.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NumericPeakCore, type NumericSink } from "../src/numeric-peak-core.ts";
import type { Scale, Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

const SCALE = JSON.parse(
  readFileSync(join(ASSETS, "scales/digital_peak.json"), "utf8"),
) as Scale;
const SKIN = JSON.parse(
  readFileSync(join(ASSETS, "skins/digital_studio_numeric.json"), "utf8"),
) as Skin;

interface RectOp {
  x: number;
  y: number;
  w: number;
  h: number;
  color: string;
}
interface TextOp {
  x: number;
  y: number;
  text: string;
  color: string;
}

function recorder(): NumericSink & { rects: RectOp[]; texts: TextOp[] } {
  const rects: RectOp[] = [];
  const texts: TextOp[] = [];
  return {
    rects,
    texts,
    fillRect(x, y, w, h, color) {
      rects.push({ x, y, w, h, color });
    },
    drawText(x, y, text, color) {
      texts.push({ x, y, text, color });
    },
  };
}

test("rejects skins with non-numeric meter_type", () => {
  const bargraphSkin = JSON.parse(
    readFileSync(
      join(ASSETS, "skins/broadcast_classic_bargraph.json"),
      "utf8",
    ),
  ) as Skin;
  assert.throws(
    () => new NumericPeakCore({ scale: SCALE, skin: bargraphSkin }),
    /meter_type 'bargraph'/,
  );
});

test("draws 1 background fill + 2 text lines", () => {
  const core = new NumericPeakCore({ scale: SCALE, skin: SKIN });
  const sink = recorder();
  core.draw(sink, 0, 0, 220, 88);
  assert.equal(sink.rects.length, 1);
  assert.equal(sink.texts.length, 2);
  assert.ok(
    sink.texts[1].text.startsWith("PK"),
    `second line should start with PK, got "${sink.texts[1].text}"`,
  );
});

test("reading text matches Rust formatting on -3.0 dBFS", () => {
  const core = new NumericPeakCore({ scale: SCALE, skin: SKIN });
  // DigitalPeak ballistic tracks input instantly.
  core.update(-3.0, 1 / 60);
  const sink = recorder();
  core.draw(sink, 0, 0, 220, 88);
  // Rust uses {:>7.1} → 7-wide right-aligned, 1 decimal.
  assert.ok(
    sink.texts[0].text.includes("-3.0"),
    `expected reading line to contain -3.0, got "${sink.texts[0].text}"`,
  );
  assert.ok(
    sink.texts[0].text.endsWith("dBFS"),
    `expected reading line to end with dBFS units, got "${sink.texts[0].text}"`,
  );
});

test("peak hold persists after signal drop", () => {
  const core = new NumericPeakCore({ scale: SCALE, skin: SKIN });
  core.update(-1.0, 1 / 60);
  // Run enough frames to make DigitalPeak's reading decay
  // significantly. Decay rate is 20 dB / 1.5 s ≈ 13.33 dB/s, so
  // 600 ms gives ~8 dB of reading decay while the 1500 ms peak-hold
  // dwell hasn't expired yet.
  for (let i = 0; i < 36; i++) {
    core.update(-100.0, 1 / 60);
  }
  assert.ok(
    core.peakDbValue() > core.readingDbValue() + 5,
    `peak (${core.peakDbValue()}) should remain ≥ 5 dB above reading (${core.readingDbValue()}) within hold dwell`,
  );
});
