// Cross-runtime parity tests against the shared fixtures committed under
// audio-meters-core/fixtures/. The Rust side generates `expected/`; this
// suite confirms that the TS port produces matching readings to within
// `TOLERANCE_DB`.
//
// Any divergence MUST be diagnosed (typically: order-of-operations or
// `Math.fround` placement) rather than masked by widening tolerance.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { ALL_BALLISTICS, BallisticState, type Ballistic } from "../src/ballistic.ts";

interface Input {
  name: string;
  description: string;
  frame_dt_s: number;
  frames: number[];
}

interface Expected {
  input: string;
  ballistic: string;
  frame_dt_s: number;
  readings_db: number[];
}

const TOLERANCE_DB = 1e-4;

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const FIXTURES = join(__dirname, "..", "..", "fixtures");

function loadInputs(): Input[] {
  const dir = join(FIXTURES, "inputs");
  const files = readdirSync(dir)
    .filter((n) => n.endsWith(".json"))
    .sort();
  return files.map((f) => JSON.parse(readFileSync(join(dir, f), "utf8")) as Input);
}

function loadExpected(inputName: string, ballistic: Ballistic): Expected {
  const path = join(FIXTURES, "expected", `${inputName}__${ballistic}.json`);
  return JSON.parse(readFileSync(path, "utf8")) as Expected;
}

function runBallistic(kind: Ballistic, input: Input): number[] {
  const s = new BallisticState(kind);
  return input.frames.map((db) => s.update(db, input.frame_dt_s));
}

const inputs = loadInputs();

for (const input of inputs) {
  for (const ballistic of ALL_BALLISTICS) {
    test(`parity: ${input.name} / ${ballistic}`, () => {
      const got = runBallistic(ballistic, input);
      const expected = loadExpected(input.name, ballistic);
      assert.equal(expected.readings_db.length, got.length);
      for (let i = 0; i < got.length; i++) {
        const want = expected.readings_db[i];
        const have = got[i];
        const delta = Math.abs(have - want);
        assert.ok(
          delta <= TOLERANCE_DB,
          `${input.name}/${ballistic} frame ${i}: got=${have} want=${want} Δ=${delta}`,
        );
      }
    });
  }
}
