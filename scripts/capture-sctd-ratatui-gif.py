#!/usr/bin/env python3
"""Capture the deterministic SCTD-04 native-to-Ratatui promotional GIF."""

from __future__ import annotations

import re
import socket
import subprocess
from argparse import ArgumentParser
from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "docs/media/ratatui-rlvgl-dining-philosophers-full-table.gif"
READY = re.compile(r"PLAYIT_READY tcp://127\.0\.0\.1:(\d+)")
PIXEL = re.compile(rb"[0-9A-F]{8}")
WIDTH = 800
HEIGHT = 480


class PlayitClient:
    """Buffered playit connection with full-frame ARGB capture support."""

    def __init__(self, port: int) -> None:
        self.socket = socket.create_connection(("127.0.0.1", port), timeout=10)
        self.socket.settimeout(30)
        self.reader = self.socket.makefile("rb")

    def close(self) -> None:
        self.reader.close()
        self.socket.close()

    def line(self) -> bytes:
        line = self.reader.readline()
        if not line:
            raise RuntimeError("playit connection closed during capture")
        return line.strip()

    def command(self, command: str) -> str:
        self.socket.sendall(command.encode("ascii") + b"\n")
        return self.line().decode("ascii")

    def tap(self, tag: str, x: int, y: int) -> None:
        response = self.command(f"T@{tag}:{x},{y}")
        if response != "OK":
            raise RuntimeError(f"tap {tag} failed: {response}")

    def frame(self) -> Image.Image:
        image = Image.new("RGB", (WIDTH, HEIGHT))
        for y in range(0, HEIGHT, 40):
            for x in range(0, WIDTH, 40):
                width = min(40, WIDTH - x)
                height = min(40, HEIGHT - y)
                tile = self._tile(x, y, width, height)
                image.paste(tile, (x, y))
        return image

    def _tile(self, x: int, y: int, width: int, height: int) -> Image.Image:
        if self.command(f"D{x},{y},{width},{height},1") != "DUMP:queued":
            raise RuntimeError(f"playit dump was not queued at ({x},{y})")

        rgb = bytearray()
        pixel_count = 0
        while True:
            line = self.line()
            if line == b"END":
                break
            for token in PIXEL.findall(line):
                argb = int(token, 16)
                rgb.extend(((argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF))
                pixel_count += 1

        expected = width * height
        if pixel_count != expected:
            raise RuntimeError(
                f"tile ({x},{y}) contained {pixel_count} pixels, expected {expected}"
            )
        return Image.frombytes("RGB", (width, height), bytes(rgb))


def wait_for_ready(process: subprocess.Popen[str]) -> int:
    """Read the simulator readiness line and return its ephemeral TCP port."""
    assert process.stdout is not None
    for _ in range(200):
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("SCTD simulator exited before playit became ready")
        match = READY.fullmatch(line.strip())
        if match:
            return int(match.group(1))
    raise RuntimeError("SCTD simulator did not publish a playit endpoint")


def capture_sequence(client: PlayitClient) -> list[Image.Image]:
    """Capture full-table native and Ratatui Depart/Arrive sequences."""
    launch = "sctd.hero.launch"
    close = "sctd.hero.close"
    arrive = "sctd.hero.arrive"

    # Establish a deterministic paused, empty machine. These setup frames are
    # intentionally not included in the promotional sequence.
    client.tap(launch, 610, 420)
    client.tap("sctd.hero.pause", 588, 435)
    client.tap("sctd.hero.reset", 461, 435)
    for _ in range(5):
        client.tap(arrive, 80, 435)
    client.tap(close, 756, 34)

    frames = [client.frame()]

    # Native beat 2: one philosopher departs from the full table.
    client.tap(launch, 610, 420)
    client.tap("sctd.hero.depart", 207, 435)
    client.tap(close, 756, 34)
    frames.append(client.frame())

    # Native beat 3: the fifth seat is filled again.
    client.tap(launch, 610, 420)
    client.tap(arrive, 80, 435)
    client.tap(close, 756, 34)
    frames.append(client.frame())

    # Ratatui beat 1 inherits the exact native machine state.
    client.tap(launch, 610, 420)
    frames.append(client.frame())

    # Ratatui beats 2 and 3 visibly mutate the retained table in place.
    client.tap("sctd.hero.depart", 207, 435)
    frames.append(client.frame())
    client.tap(arrive, 80, 435)
    frames.append(client.frame())
    return frames


def main() -> int:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    arguments = parser.parse_args()

    process = subprocess.Popen(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "rlvgl-example-disco-sim",
            "--bin",
            "rlvgl-sctd-sim",
            "--",
            "--automation-headless",
            "--playit-port=0",
        ],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    client = None
    try:
        client = PlayitClient(wait_for_ready(process))
        frames = capture_sequence(client)
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        frames[0].save(
            arguments.output,
            save_all=True,
            append_images=frames[1:],
            duration=[1100, 900, 1200, 900, 900, 1400],
            loop=0,
            disposal=2,
            optimize=True,
        )
        print(f"SCTD-04 GIF: {arguments.output}")
        return 0
    finally:
        if client is not None:
            client.close()
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
