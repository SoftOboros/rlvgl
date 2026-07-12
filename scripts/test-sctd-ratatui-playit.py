#!/usr/bin/env python3
"""Playit smoke test for the SCTD-04 additive Ratatui hero window."""

from __future__ import annotations

import os
import re
import socket
import subprocess
import sys
from argparse import ArgumentParser
from pathlib import Path
from urllib.parse import urlparse


ROOT = Path(__file__).resolve().parents[1]
READY = re.compile(r"PLAYIT_READY tcp://127\.0\.0\.1:(\d+)")


def read_line(stream) -> str:
    line = stream.readline()
    if not line:
        raise RuntimeError("SCTD simulator exited before Playit became ready")
    return line.strip()


def recv_line(connection: socket.socket) -> str:
    data = bytearray()
    while not data.endswith(b"\n"):
        chunk = connection.recv(1)
        if not chunk:
            raise RuntimeError("Playit connection closed")
        if chunk != b"\r":
            data.extend(chunk)
    return data.decode("ascii").strip()


def command(connection: socket.socket, text: str) -> str:
    connection.sendall(text.encode("ascii") + b"\n")
    return recv_line(connection)


def dump_region(connection: socket.socket, x: int, y: int, width: int, height: int) -> str:
    if command(connection, f"D{x},{y},{width},{height},1") != "DUMP:queued":
        raise AssertionError("framebuffer dump was not queued")
    lines = []
    while True:
        line = recv_line(connection)
        if line == "END":
            return " ".join(lines)
        lines.append(line)


def main() -> int:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument(
        "--uri",
        help="Use an existing Playit TCP bridge, for example tcp://127.0.0.1:5570",
    )
    parser.add_argument(
        "--leave-open",
        action="store_true",
        help="Leave the hero popup visible after verification for bench review",
    )
    arguments = parser.parse_args()

    environment = os.environ.copy()
    environment["RUSTFLAGS"] = ""
    process = None
    try:
        if arguments.uri:
            parsed = urlparse(arguments.uri)
            if parsed.scheme != "tcp" or parsed.hostname is None or parsed.port is None:
                raise ValueError("--uri must be a tcp://host:port URI")
            host = parsed.hostname
            port = parsed.port
        else:
            host = "127.0.0.1"
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
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
            )
            assert process.stdout is not None
            port = None
            for _ in range(200):
                match = READY.fullmatch(read_line(process.stdout))
                if match:
                    port = int(match.group(1))
                    break
            if port is None:
                raise RuntimeError("missing PLAYIT_READY line")

        with socket.create_connection((host, port), timeout=5) as connection:
            connection.settimeout(30 if arguments.uri else 5)
            tags = [
                "sctd.hero.launch",
                "sctd.hero.window",
                "sctd.hero.content",
                "sctd.hero.close",
                "sctd.hero.arrive",
                "sctd.hero.depart",
                "sctd.hero.panic",
                "sctd.hero.reset",
                "sctd.hero.pause",
                "sctd.hero.speed",
            ]
            for tag in tags:
                response = command(connection, f"QE:{tag}")
                if response != "EXISTS:1":
                    raise AssertionError(f"missing {tag}: {response}")

            if command(connection, "T@sctd.hero.launch:610,420") != "OK":
                raise AssertionError("hero launcher did not accept Playit tap")
            if command(connection, "T@sctd.hero.arrive:40,435") != "OK":
                raise AssertionError("native Arrive control did not accept Playit tap")

            table_sample = (520, 210)
            region = dump_region(connection, *table_sample, 40, 10)
            if "FF5C3920" not in region:
                colors = sorted(set(re.findall(r"[0-9A-F]{8}", region)))
                raise AssertionError(
                    f"Ratatui dining-table pixels absent from hero content dump: {colors[:12]}"
                )

            if not arguments.leave_open:
                if command(connection, "T@sctd.hero.close:760,24") != "OK":
                    raise AssertionError("native close control did not accept Playit tap")
            status = command(connection, "?")
            if not status.startswith("STAT:"):
                raise AssertionError(f"unexpected status response: {status}")

        print(f"SCTD-04 Playit hero smoke: PASS (table sample {table_sample[0]},{table_sample[1]})")
        return 0
    finally:
        if process is not None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:  # noqa: BLE001 - command-line gate reports one concise failure.
        print(f"SCTD-04 Playit hero smoke: FAIL: {error}", file=sys.stderr)
        raise
