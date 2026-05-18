// Tests for RelativelyGatedLufsI. Mirror of audio-meters-core/src/integrated.rs.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  RelativelyGatedLufsI,
  ABSOLUTE_GATE_DB,
  NEG_INFINITY_FLOOR_DB,
} from "../src/integrated.ts";

test("constructor rejects invalid window sizes", () => {
  assert.throws(() => new RelativelyGatedLufsI(0), /positive integer/);
  assert.throws(() => new RelativelyGatedLufsI(-1), /positive integer/);
  assert.throws(() => new RelativelyGatedLufsI(1.5), /positive integer/);
});

test("fresh integrator reads floor", () => {
  const g = new RelativelyGatedLufsI(256);
  assert.equal(g.isEmpty(), true);
  assert.equal(g.len(), 0);
  assert.equal(g.capacity(), 256);
  assert.equal(g.readingDb(), NEG_INFINITY_FLOOR_DB);
});

test("steady input converges to input", () => {
  const g = new RelativelyGatedLufsI(256);
  for (let i = 0; i < 1000; i++) g.update(-23, 1 / 60);
  assert.ok(
    Math.abs(g.readingDb() - -23) < 0.1,
    `expected ~-23, got ${g.readingDb()}`,
  );
  assert.equal(g.len(), 256);
});

test("absolute gate excludes silence", () => {
  const g = new RelativelyGatedLufsI(512);
  for (let i = 0; i < 256; i++) g.update(-23, 1 / 60);
  for (let i = 0; i < 256; i++) g.update(-100, 1 / 60);
  assert.ok(
    Math.abs(g.readingDb() - -23) < 0.5,
    `absolute gate failed: got ${g.readingDb()}`,
  );
});

test("relative gate excludes quiet passages", () => {
  const g = new RelativelyGatedLufsI(256);
  let absSum = 0;
  let absCount = 0;
  for (let f = 0; f < 256; f++) {
    const dbfs = f % 5 === 0 ? -45 : -23;
    g.update(dbfs, 1 / 60);
    if (dbfs >= ABSOLUTE_GATE_DB) {
      absSum += Math.pow(10, dbfs / 10);
      absCount += 1;
    }
  }
  const absOnlyMean = 10 * Math.log10(absSum / absCount);
  assert.ok(
    g.readingDb() > absOnlyMean,
    `relative-gated (${g.readingDb()}) should exceed absolute-only (${absOnlyMean})`,
  );
  assert.ok(
    Math.abs(g.readingDb() - -23) < 0.2,
    `doubly-gated should track loud passage near -23, got ${g.readingDb()}`,
  );
});

test("ring wraps after windowSize samples", () => {
  const g = new RelativelyGatedLufsI(8);
  const inputs = [-100, -90, -80, -23, -23, -23, -23, -23, -23, -23, -23, -23];
  for (const x of inputs) g.update(x, 1 / 60);
  assert.equal(g.len(), 8);
  assert.ok(
    Math.abs(g.readingDb() - -23) < 0.1,
    `after wrap, reading should track recent 8: ${g.readingDb()}`,
  );
});

test("reset returns to floor", () => {
  const g = new RelativelyGatedLufsI(32);
  for (let i = 0; i < 20; i++) g.update(-10, 1 / 60);
  assert.ok(g.readingDb() > -20);
  g.reset();
  assert.equal(g.readingDb(), NEG_INFINITY_FLOOR_DB);
  assert.equal(g.len(), 0);
});

test("nonfinite input is floored", () => {
  const g = new RelativelyGatedLufsI(32);
  g.update(NaN, 1 / 60);
  g.update(-Infinity, 1 / 60);
  assert.equal(g.readingDb(), NEG_INFINITY_FLOOR_DB);
});
