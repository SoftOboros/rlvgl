#!/usr/bin/env bash
set -e

if [[ "$1" == "-h" || "$1" == "--help" ]]; then
  echo "Usage: $(basename "$0") [PORT [BAUD]]"
  echo "Default PORT=/dev/ttyACM0 BAUD=115200"
  exit 0
fi

PORT=${1:-/dev/ttyACM0}
BAUD=${2:-115200}

if ! python3 -m serial.tools.miniterm --help >/dev/null 2>&1; then
  echo "pyserial miniterm not found; install with 'pip3 install pyserial'" >&2
  exit 1
fi

exec python3 -m serial.tools.miniterm "$PORT" "$BAUD"
