// Headless tests for LufsGaugeCore. Mirror of widgets/src/meters/lufs_gauge.rs.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { LufsGaugeCore, type LufsSink } from "../src/lufs-gauge-core.ts";
import type { Scale, Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

const SCALE = JSON.parse(
  readFileSync(join(ASSETS, "scales/lufs_ebu_r128.json"), "utf8"),
) as Scale;
const SKIN = JSON.parse(
  readFileSync(join(ASSETS, "skins/lufs_ebu_r128_gauge.json"), "utf8"),
) as Skin;

interface OpRect {
  x: number;
  y: number;
  w: number;
  h: number;
  color: string;
}
interface OpText {
  x: number;
  y: number;
  text: string;
  color: string;
}

function recorder(): LufsSink & { rects: OpRect[]; texts: OpText[] } {
  const rects: OpRect[] = [];
  const texts: OpText[] = [];
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

test("rejects skins with non-lufs-gauge meter_type", () => {
  const bargraphSkin = JSON.parse(
    readFileSync(
      join(ASSETS, "skins/broadcast_classic_bargraph.json"),
      "utf8",
    ),
  ) as Skin;
  assert.throws(
    () => new LufsGaugeCore({ scale: SCALE, skin: bargraphSkin }),
    /meter_type 'bargraph'/,
  );
});

test("draws 1 background fill + 3 text lines (I, S, M)", () => {
  const g = new LufsGaugeCore({ scale: SCALE, skin: SKIN });
  const sink = recorder();
  g.draw(sink, 0, 0, 280, 140);
  assert.equal(sink.rects.length, 1);
  assert.equal(sink.texts.length, 3);
  assert.ok(sink.texts[0].text.startsWith("I "));
  assert.ok(sink.texts[1].text.startsWith("S "));
  assert.ok(sink.texts[2].text.startsWith("M "));
});

test("integrated colour is Nominal at target", () => {
  const g = new LufsGaugeCore({ scale: SCALE, skin: SKIN });
  for (let i = 0; i < 2000; i++) g.update(-23, 1 / 60);
  const sink = recorder();
  g.draw(sink, 0, 0, 280, 140);
  assert.equal(
    sink.texts[0].color,
    SKIN.palette.Nominal,
    `integrated should colour as Nominal at target, got ${sink.texts[0].color}`,
  );
  assert.ok(
    sink.texts[0].text.includes("(+0.0 LU)") ||
      sink.texts[0].text.includes("(-0.0 LU)"),
    `expected ~0 LU at target, got ${sink.texts[0].text}`,
  );
});

test("integrated colour is Hot above +1.5 LU", () => {
  const g = new LufsGaugeCore({ scale: SCALE, skin: SKIN });
  for (let i = 0; i < 2000; i++) g.update(-18, 1 / 60); // 5 LU above target
  const sink = recorder();
  g.draw(sink, 0, 0, 280, 140);
  assert.equal(sink.texts[0].color, SKIN.palette.Hot);
});
