#!/usr/bin/env python3
"""Host tests for the STM32H747I-DISCO physical capture helper."""

from __future__ import annotations

import unittest
from unittest import mock

import capture_stm32h747i_disco_pair as capture


class CaptureTests(unittest.TestCase):
    """Exercise wrapping accounting and probe-rs output parsing."""

    def test_ring_depth_wraps_as_u32(self) -> None:
        self.assertEqual(capture.ring_depth(1, 0xFFFF_FFFF), 2)

    def test_progress_requires_both_producers_and_consumers(self) -> None:
        complete = {
            "clock_ready": 1,
            "cmd_head": 1,
            "cmd_tail": 1,
            "cmd_depth": 0,
            "evt_head": 4,
            "evt_tail": 4,
            "evt_depth": 0,
        }
        self.assertTrue(capture.progressed(complete))
        for field in ("cmd_head", "cmd_tail", "evt_head", "evt_tail"):
            incomplete = complete | {field: 0}
            self.assertFalse(capture.progressed(incomplete), field)

    def test_progress_rejects_depth_beyond_candidate_capacity(self) -> None:
        complete = {
            "clock_ready": 1,
            "cmd_head": 17,
            "cmd_tail": 0,
            "cmd_depth": 17,
            "evt_head": 1,
            "evt_tail": 1,
            "evt_depth": 0,
        }
        self.assertFalse(capture.progressed(complete))

    @mock.patch.object(
        capture,
        "run",
        return_value="read 30047000\n0000000a 00000009",
    )
    def test_read_words_uses_trailing_probe_values(self, _run: mock.Mock) -> None:
        self.assertEqual(
            capture.read_words(
                chip="STM32H747XIHx",
                probe="0483:3754:test",
                speed=1000,
                address=capture.MAILBOX_BASE,
                words=2,
            ),
            [10, 9],
        )


if __name__ == "__main__":
    unittest.main()
