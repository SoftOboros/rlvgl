// SPDX-License-Identifier: MIT
// UEFI-specific playit automation tests for the disco demo.
//
// These tests connect to an already-running QEMU instance via the playit
// serial chardev exposed as a TCP socket. Set RLVGL_UEFI_PLAYIT_PORT to
// the port number.

import test from 'node:test';
import assert from 'node:assert/strict';

import { connectDiscoSession } from '../src/index.js';
import { dumpHasVisiblePixels, assertAllTagsExist } from './shared-assertions.js';

const PLAYIT_PORT = process.env.RLVGL_UEFI_PLAYIT_PORT;

if (!PLAYIT_PORT) {
  test('uefi tests skipped (RLVGL_UEFI_PLAYIT_PORT not set)', { skip: true }, () => {});
} else {
  // Share a single session across all UEFI tests to avoid reconnection overhead.
  // The UEFI target's handshake only runs once at boot.
  test('uefi disco playit test suite', async (t) => {
    const session = await connectDiscoSession({
      port: Number(PLAYIT_PORT),
      commandTimeoutMs: 10_000,
      waitForReady: true,
      readyTimeoutMs: 30_000,
      readySettleMs: 6000,
    });

    try {
      await t.test('reports advancing status over playit serial', async () => {
        const before = await session.status();
        assert(before.tickCount >= 0);
        assert(before.presentCount >= 0);

        const after = await session.tick(2);
        assert(after.tickCount > before.tickCount, 'tick count did not advance');
        assert(after.presentCount > before.presentCount, 'present count did not advance');
      });

      await t.test('widget tree has all expected tags', async () => {
        await assertAllTagsExist(session);
      });

      await t.test('keyboard navigation works through playit', { todo: 'EDK2 ConIn echoes characters to ConOut, corrupting playit responses. Needs UART echo suppression.' }, async () => {
        assert.equal(await session.widget('disco.settings.audio').isVisible(), false);
        await session.keyDown('s');
        assert.equal(await session.widget('disco.settings.audio').isVisible(), true);
      });

      await t.test('framebuffer dump produces non-blank pixels', async () => {
        const dump = await session.dumpRect({ x: 0, y: 0, width: 8, height: 4, frames: 1 });
        assert.equal(dump.frames.length, 1);
        assert(dumpHasVisiblePixels(dump), 'framebuffer was blank');
      });
    } finally {
      await session.close();
    }
  });
}
