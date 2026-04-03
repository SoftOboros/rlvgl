#!/usr/bin/env python3
"""Serial monitor for STM32H747I-DISCO debug UART.

Reads from the ST-LINK VCP or a specified serial port and prints output.
Usage:
    python3 tools/serial_monitor.py [--port /dev/cu.usbmodem1403] [--baud 115200] [--timeout 5]
"""

import argparse
import glob
import sys
import time

def find_port():
    """Auto-detect ST-LINK VCP port."""
    candidates = glob.glob("/dev/cu.usbmodem*")
    if candidates:
        return candidates[0]
    candidates = glob.glob("/dev/ttyACM*")
    if candidates:
        return candidates[0]
    return None

def monitor(port, baud, timeout, reset=False):
    try:
        import serial
    except ImportError:
        print("pyserial not installed — run: pip3 install pyserial", file=sys.stderr)
        sys.exit(1)

    if port is None:
        port = find_port()
    if port is None:
        print("No serial port found. Specify with --port", file=sys.stderr)
        sys.exit(1)

    print(f"Opening {port} @ {baud} baud (timeout={timeout}s)")
    try:
        s = serial.Serial(port, baud, timeout=1)
    except Exception as e:
        print(f"Failed to open {port}: {e}", file=sys.stderr)
        sys.exit(1)

    s.reset_input_buffer()
    buf = b""
    end = time.time() + timeout
    try:
        while time.time() < end:
            chunk = s.read(s.in_waiting or 1)
            if chunk:
                buf += chunk
                # Reset timeout on new data
                end = time.time() + 2
    except KeyboardInterrupt:
        pass
    finally:
        s.close()

    text = buf.decode("utf-8", errors="replace")
    if text.strip():
        print(text)
    else:
        print("(no output received)")
    return text

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="STM32H747I-DISCO serial monitor")
    parser.add_argument("--port", default=None, help="Serial port (auto-detect if omitted)")
    parser.add_argument("--baud", type=int, default=115200, help="Baud rate (default: 115200)")
    parser.add_argument("--timeout", type=int, default=5, help="Read timeout in seconds (default: 5)")
    args = parser.parse_args()
    monitor(args.port, args.baud, args.timeout)
