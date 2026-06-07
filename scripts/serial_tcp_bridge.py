#!/usr/bin/env python3
"""
Serial-to-TCP bridge for the playit test harness.

Listens on a TCP port and forwards bytes between the socket and a serial
port (e.g., the STM32H747I-DISCO's ST-Link VCP). Lets the existing
playit Node.js client connect via TCP to a hardware target.

Usage:
  serial_tcp_bridge.py --port /dev/cu.usbmodem1302 --baud 115200 --tcp 5570

Prints "BRIDGE_READY <port>" on stdout when the TCP listener is up.
"""
import argparse
import select
import socket
import sys

import serial


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", required=True, help="serial device path")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--tcp", type=int, default=0, help="tcp port (0 = auto)")
    parser.add_argument("--host", default="127.0.0.1")
    args = parser.parse_args()

    ser = serial.Serial(args.port, args.baud, timeout=0)
    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind((args.host, args.tcp))
    listener.listen(1)
    bound_port = listener.getsockname()[1]
    print(f"BRIDGE_READY tcp://{args.host}:{bound_port}", flush=True)

    try:
        while True:
            client, _ = listener.accept()
            client.setblocking(False)
            # Drain any stale serial data that accumulated while no client was connected
            while ser.in_waiting:
                ser.read(ser.in_waiting)
            print(f"BRIDGE_CONNECT", flush=True)
            try:
                while True:
                    rlist, _, _ = select.select([client, ser], [], [], 0.05)
                    if client in rlist:
                        try:
                            data = client.recv(1024)
                        except (BlockingIOError, ConnectionResetError):
                            data = b""
                        if not data:
                            break
                        ser.write(data)
                    if ser in rlist or ser.in_waiting:
                        chunk = ser.read(ser.in_waiting or 1)
                        if chunk:
                            try:
                                client.sendall(chunk)
                            except (BrokenPipeError, ConnectionResetError):
                                break
            finally:
                client.close()
                print("BRIDGE_DISCONNECT", flush=True)
    except KeyboardInterrupt:
        pass
    finally:
        listener.close()
        ser.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
