#!/usr/bin/env bash
#
# Run playit automation tests against the STM32H747I-DISCO over its
# ST-Link USB CDC ACM serial port.  Bridges the serial device to a TCP
# socket via scripts/serial_tcp_bridge.py, then runs the Node.js tests.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERIAL_PORT="${SERIAL_PORT:-}"
BAUD="${BAUD:-115200}"

if [[ -z "$SERIAL_PORT" ]]; then
  for candidate in /dev/cu.usbmodem*; do
    if [[ -e "$candidate" ]]; then
      SERIAL_PORT="$candidate"
      break
    fi
  done
fi

if [[ -z "$SERIAL_PORT" || ! -e "$SERIAL_PORT" ]]; then
  echo "No ST-Link serial port found. Set SERIAL_PORT explicitly." >&2
  exit 1
fi

echo "Using serial port: $SERIAL_PORT"

# Pick a free TCP port
HW_PORT=$(python3 -c "import socket; s=socket.socket(); s.bind(('',0)); print(s.getsockname()[1]); s.close()")

# Start the bridge in the background
python3 "$ROOT_DIR/scripts/serial_tcp_bridge.py" \
  --port "$SERIAL_PORT" --baud "$BAUD" --tcp "$HW_PORT" &
BRIDGE_PID=$!

cleanup() {
  if kill -0 "$BRIDGE_PID" >/dev/null 2>&1; then
    kill "$BRIDGE_PID" >/dev/null 2>&1 || true
    wait "$BRIDGE_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# Wait for bridge to be ready
for _ in $(seq 1 50); do
  if nc -z 127.0.0.1 "$HW_PORT" 2>/dev/null; then
    break
  fi
  sleep 0.1
done

if ! nc -z 127.0.0.1 "$HW_PORT" 2>/dev/null; then
  echo "Bridge never became ready on port $HW_PORT" >&2
  exit 1
fi

echo "Bridge ready on tcp://127.0.0.1:$HW_PORT"
echo "Running STM32H747I-DISCO playit tests..."

RLVGL_DISCO_HW_PORT="$HW_PORT" \
  node --test "$ROOT_DIR/playit/node/test/stm32h747i-disco.test.js"

echo "STM32H747I-DISCO playit tests passed."
