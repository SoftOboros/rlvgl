// SPDX-License-Identifier: MIT
// Node-level automation tests for the disco simulator playit harness.

import test from 'node:test';
import assert from 'node:assert/strict';

import { launchDiscoSim } from '../src/index.js';
import { dumpHasVisiblePixels, dumpSignature, assertAllTagsExist } from './shared-assertions.js';

test('headless disco sim reports advancing status and exposes frame dumps', async () => {
  const session = await launchDiscoSim({
    cwd: process.cwd(),
    automationHeadless: true
  });

  try {
    const before = await session.status();
    const after = await session.tick();
    assert(after.tickCount > before.tickCount);
    assert(after.presentCount > before.presentCount);

    const root = session.widget('disco.root');
    assert.equal(await root.exists(), true);

    const dump = await session.dumpRect({ x: 0, y: 0, width: 6, height: 4, frames: 1 });
    assert.equal(dump.frames.length, 1);
    assert.equal(dump.frames[0].length, 4);
    assert(dumpHasVisiblePixels(dump));
  } finally {
    await session.close();
  }
});

test('settings hotspot tap opens the settings wing through the shared socket path', async () => {
  const session = await launchDiscoSim({
    cwd: process.cwd(),
    automationHeadless: true
  });

  try {
    const audio = session.widget('disco.settings.audio');
    assert.equal(await audio.isVisible(), false);

    await session.widget('disco.main.settings').tap();
    assert.equal(await audio.isVisible(), true);

    const dump = await audio.dump({ width: 8, height: 8 });
    assert(dumpHasVisiblePixels(dump));
  } finally {
    await session.close();
  }
});

test('info hotspot tap changes the event log framebuffer content', async () => {
  const session = await launchDiscoSim({
    cwd: process.cwd(),
    automationHeadless: true
  });

  try {
    const events = session.widget('disco.events');
    const before = await events.dump({ width: 40, height: 20 });

    await session.widget('disco.main.info').tap();
    assert.equal(await session.widget('disco.info.live_stats').isVisible(), true);
    await session.widget('disco.info.live_stats').tap();

    const after = await events.dump({ width: 40, height: 20 });
    assert.notEqual(dumpSignature(after), dumpSignature(before));
  } finally {
    await session.close();
  }
});

test(
  'windowed mode exposes the same playit socket surface',
  {
    skip: !process.env.RLVGL_WINDOWED_SMOKE
  },
  async () => {
    const session = await launchDiscoSim({
      cwd: process.cwd(),
      automationHeadless: false
    });

    try {
      const status = await session.tick();
      assert(status.presentCount > 0);
      assert.equal(await session.widget('disco.root').exists(), true);
    } finally {
      await session.close();
    }
  }
);
