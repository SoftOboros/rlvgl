// SPDX-License-Identifier: MIT
// Hardware playit automation tests for the STM32H747I-DISCO target.
//
// Connects to a serial-to-TCP bridge that exposes the board's USART1
// (ST-Link VCP) as a TCP socket.  Set RLVGL_DISCO_HW_PORT to the bridge
// port to enable these tests; otherwise they skip gracefully.

import test from 'node:test';
import assert from 'node:assert/strict';

import { connectDiscoSession } from '../src/index.js';
import { dumpHasVisiblePixels, KNOWN_TAGS } from './shared-assertions.js';

const HW_PORT = process.env.RLVGL_DISCO_HW_PORT;

if (!HW_PORT) {
  test('hardware tests skipped (RLVGL_DISCO_HW_PORT not set)', { skip: true }, () => {});
} else {
  test('stm32h747i-disco playit test suite', async (t) => {
    const session = await connectDiscoSession({
      port: Number(HW_PORT),
      commandTimeoutMs: 8_000,
    });

    try {
      await t.test('reports advancing tick count over USART1', async () => {
        const before = await session.status();
        assert(before.tickCount >= 0);
        assert(before.presentCount >= 0);

        // The firmware only renders on dirty frames, so present_count may
        // stall. Tick count, however, advances every loop iteration.
        await new Promise((r) => setTimeout(r, 200));
        const after = await session.status();
        assert(after.tickCount > before.tickCount, 'tick count did not advance');
      });

      await t.test('framebuffer dump produces non-blank pixels', async () => {
        const dump = await session.dumpRect({ x: 0, y: 0, width: 16, height: 8, frames: 1 });
        assert.equal(dump.frames.length, 1);
        assert(dumpHasVisiblePixels(dump), 'framebuffer was blank');
      });
    } finally {
      await session.close();
    }
  });
}
