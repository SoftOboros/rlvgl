// SPDX-License-Identifier: MIT
// CRATES-CI-02 — node automation test for the user-sim Consumer Project.
//
// Drives the crates-built consumer binary through playit/node UNMODIFIED
// (CRATES-CI-00 §10 / INV-C7): the same client that drives the in-tree
// disco-sim must launch and drive a downstream user's simulator. The
// binary path is injected via RLVGL_USER_SIM_BIN (no silent fallback to
// the in-tree cargo run path, which would defeat the gate).

import test from 'node:test';
import assert from 'node:assert/strict';

import { launchDiscoSim } from '../../../playit/node/src/index.js';

const SCREEN = { width: 480, height: 320 };

function binaryPath() {
  const bin = process.env.RLVGL_USER_SIM_BIN;
  assert.ok(
    bin,
    'RLVGL_USER_SIM_BIN must point at the built crates-ci-user-sim binary'
  );
  return bin;
}

function launch() {
  return launchDiscoSim({
    binaryPath: binaryPath(),
    automationHeadless: true,
    screen: SCREEN
  });
}

function dumpSignature(dump) {
  return JSON.stringify(dump.frames);
}

function dumpHasVisiblePixels(dump) {
  return dump.frames.some((frame) =>
    frame.some((row) => row.some((pixel) => pixel !== 0))
  );
}

test('user sim reports advancing status and exposes its tagged tree', async () => {
  const session = await launch();

  try {
    const before = await session.status();
    const after = await session.tick();
    assert.ok(after.tickCount > before.tickCount);
    assert.ok(after.presentCount > before.presentCount);

    for (const tag of ['user.root', 'user.title', 'user.button']) {
      assert.equal(await session.widget(tag).exists(), true, `missing tag: ${tag}`);
    }
    assert.equal(await session.widget('user.nope').exists(), false);

    const rootBounds = await session.widget('user.root').bounds();
    assert.deepEqual(rootBounds, { x: 0, y: 0, width: 480, height: 320 });

    const dump = await session.dumpRect({ x: 0, y: 0, width: 6, height: 4, frames: 1 });
    assert.equal(dump.frames.length, 1);
    assert.equal(dump.frames[0].length, 4);
    assert.ok(dumpHasVisiblePixels(dump));
  } finally {
    await session.close();
  }
});

test('button tap visibly changes the framebuffer content', async () => {
  const session = await launch();

  try {
    const button = session.widget('user.button');
    assert.equal(await button.isVisible(), true);

    const before = await button.dump({ width: 24, height: 12 });
    await button.tap(); // QB -> center coords -> T@user.button:<x>,<y>
    const after = await button.dump({ width: 24, height: 12 });

    assert.notEqual(
      dumpSignature(after),
      dumpSignature(before),
      'tap did not change the button pixels (text/bg state change missing)'
    );
  } finally {
    await session.close();
  }
});
