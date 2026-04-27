// Kitchen-sink integration test for the TS audio-meters widget tree.
// Mirror of widgets/tests/audio_meters_integration.rs.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { LedBargraphCore } from "../src/led-bargraph-core.ts";
import { NeedleVuCore } from "../src/needle-vu-core.ts";
import { NumericPeakCore } from "../src/numeric-peak-core.ts";
import { StereoPair, splitHorizontal } from "../src/stereo.ts";
import type { Scale, Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

const VU_BROADCAST = JSON.parse(
  readFileSync(join(ASSETS, "scales/vu_broadcast.json"), "utf8"),
) as Scale;
const DIGITAL_PEAK = JSON.parse(
  readFileSync(join(ASSETS, "scales/digital_peak.json"), "utf8"),
) as Scale;
const SKIN_BARGRAPH = JSON.parse(
  readFileSync(join(ASSETS, "skins/broadcast_classic_bargraph.json"), "utf8"),
) as Skin;
const SKIN_NEEDLE = JSON.parse(
  readFileSync(join(ASSETS, "skins/broadcast_classic_needle.json"), "utf8"),
) as Skin;
const SKIN_NUMERIC = JSON.parse(
  readFileSync(join(ASSETS, "skins/digital_studio_numeric.json"), "utf8"),
) as Skin;
const SKIN_DIGITAL_BARGRAPH = JSON.parse(
  readFileSync(join(ASSETS, "skins/digital_studio_bargraph.json"), "utf8"),
) as Skin;

function syntheticSignal(n: number): number {
  if (n < 60) return -120;
  if (n < 120) return -60 + ((n - 60) / 60) * 50;
  if (n < 180) return -10;
  if (n === 180) return -1;
  return -120;
}

interface CountSink {
  rects: number;
  texts: number;
  fillRect(): void;
  drawText(): void;
}
function counter(): CountSink {
  return {
    rects: 0,
    texts: 0,
    fillRect() {
      this.rects++;
    },
    drawText() {
      this.texts++;
    },
  };
}

test("one of each TS core renders through full sequence", () => {
  const bar = new LedBargraphCore({
    scale: VU_BROADCAST,
    skin: SKIN_BARGRAPH,
    showTicks: true,
  });
  const needle = new NeedleVuCore({
    scale: VU_BROADCAST,
    skin: SKIN_NEEDLE,
    showTicks: true,
  });
  const numeric = new NumericPeakCore({
    scale: DIGITAL_PEAK,
    skin: SKIN_NUMERIC,
  });
  const outer = { x: 0, y: 0, w: 96, h: 320 };
  const [lb, rb] = splitHorizontal(outer, 4);
  const stereoLeft = new LedBargraphCore({
    scale: DIGITAL_PEAK,
    skin: SKIN_DIGITAL_BARGRAPH,
  });
  const stereoRight = new LedBargraphCore({
    scale: DIGITAL_PEAK,
    skin: SKIN_DIGITAL_BARGRAPH,
  });
  const stereo = new StereoPair(outer, 4, stereoLeft, stereoRight);
  void lb;
  void rb;

  const dt = 1 / 60;
  let midBar = -Infinity;
  let midNeedle = -Infinity;
  let midLeft = -Infinity;
  let midRight = -Infinity;

  for (let f = 0; f < 480; f++) {
    const dbfs = syntheticSignal(f);
    bar.update(dbfs, dt);
    needle.update(dbfs, dt);
    numeric.update(dbfs, dt);
    stereo.updateStereo(dbfs, dbfs - 6, dt);

    if (f === 175) {
      midBar = bar.readingDbValue();
      midNeedle = needle.readingDbValue();
      midLeft = stereo.left.readingDbValue();
      midRight = stereo.right.readingDbValue();
    }

    if (f % 30 === 0) {
      const c = counter();
      bar.draw(c, 0, 0, 96, 320);
      assert.ok(c.rects > 0, `frame ${f}: bargraph drew nothing`);
      assert.ok(c.texts > 0, `frame ${f}: bargraph ticks drew no text`);

      const c2 = counter();
      needle.draw(c2, 100, 0, 320, 200);
      assert.ok(c2.rects > 0, `frame ${f}: needle drew nothing`);
      assert.ok(c2.texts > 0, `frame ${f}: needle ticks drew no text`);

      const c3 = counter();
      numeric.draw(c3, 100, 220, 220, 88);
      assert.equal(c3.rects, 1, `frame ${f}: numeric bg`);
      assert.equal(c3.texts, 2, `frame ${f}: numeric text lines`);

      const c4 = counter();
      stereo.draw(c4);
      assert.ok(c4.rects > 0, `frame ${f}: stereo drew nothing`);
    }
  }

  assert.ok(
    midBar > -15 && midBar < -5,
    `VU bargraph at mid-plateau (${midBar}) should track -10 dBFS`,
  );
  assert.ok(
    midNeedle > -15 && midNeedle < -5,
    `VU needle at mid-plateau (${midNeedle}) should track -10 dBFS`,
  );
  assert.ok(
    midLeft > midRight + 3,
    `stereo asymmetry at plateau: left ${midLeft}, right ${midRight}`,
  );

  const finalNumPeak = numeric.peakDbValue();
  assert.ok(
    finalNumPeak > -90 && finalNumPeak <= 0,
    `numeric peak after sequence: ${finalNumPeak}`,
  );
});

test("setBallistic on a running widget resets to floor", () => {
  const bar = new LedBargraphCore({
    scale: VU_BROADCAST,
    skin: SKIN_BARGRAPH,
  });
  for (let i = 0; i < 120; i++) {
    bar.update(-10, 1 / 60);
  }
  const before = bar.readingDbValue();
  bar.setBallistic("DigitalPeak");
  const after = bar.readingDbValue();
  assert.ok(
    after < before,
    `swap should reset reading: before ${before}, after ${after}`,
  );
  bar.update(-10, 1 / 60);
  const oneStep = bar.readingDbValue();
  assert.ok(
    Math.abs(oneStep - -10) < 0.5,
    `DigitalPeak should track input quickly: ${oneStep}`,
  );
});
