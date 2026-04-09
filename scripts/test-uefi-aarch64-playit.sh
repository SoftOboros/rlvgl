#!/usr/bin/env bash
#
# Headless playit automation test for the AArch64 UEFI disco demo.
# Boots under QEMU with a PCI-serial playit chardev, waits for the TCP
# socket to accept connections, runs Node.js tests, then tears down QEMU.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MONITOR_SOCK="${MONITOR_SOCK:-/tmp/rlvgl-uefi-playit-monitor.sock}"
QEMU_BIN="${QEMU_BIN:-qemu-system-aarch64}"
UEFI_CODE="${UEFI_CODE:-}"
UEFI_VARS="${UEFI_VARS:-}"
TARGET_TRIPLE="${TARGET_TRIPLE:-aarch64-unknown-uefi}"

if [[ -z "$UEFI_CODE" && -f /opt/homebrew/share/qemu/edk2-aarch64-code.fd ]]; then
  UEFI_CODE=/opt/homebrew/share/qemu/edk2-aarch64-code.fd
fi
if [[ -z "$UEFI_VARS" && -f /opt/homebrew/share/qemu/edk2-arm-vars.fd ]]; then
  UEFI_VARS=/opt/homebrew/share/qemu/edk2-arm-vars.fd
fi

if ! command -v "$QEMU_BIN" >/dev/null 2>&1; then
  echo "Missing QEMU binary: $QEMU_BIN — skipping UEFI playit tests." >&2
  exit 0
fi

if [[ -z "$UEFI_CODE" || -z "$UEFI_VARS" ]]; then
  echo "Missing EDK2 firmware — skipping UEFI playit tests." >&2
  exit 0
fi

# Find a free TCP port for the playit chardev
PLAYIT_PORT=$(python3 -c "import socket; s=socket.socket(); s.bind(('',0)); print(s.getsockname()[1]); s.close()")

rm -f "$MONITOR_SOCK"

cd "$ROOT_DIR"
mkdir -p target/uefi-esp/EFI/BOOT
cp "$UEFI_VARS" target/AAVMF_VARS.fd

cargo build --manifest-path "$ROOT_DIR/examples/uefi-disco/Cargo.toml" --target "$TARGET_TRIPLE"
cp "examples/uefi-disco/target/$TARGET_TRIPLE/debug/rlvgl-uefi-disco.efi" target/uefi-esp/EFI/BOOT/BOOTAA64.EFI

echo "Launching QEMU with playit chardev on port $PLAYIT_PORT..."

"$QEMU_BIN" \
  -machine virt \
  -cpu cortex-a72 \
  -m 512 \
  -display none \
  -serial tcp:127.0.0.1:"$PLAYIT_PORT",server=on,wait=off \
  -monitor "unix:$MONITOR_SOCK,server,nowait" \
  -device ramfb \
  -drive if=pflash,format=raw,readonly=on,file="$UEFI_CODE" \
  -drive if=pflash,format=raw,file="$ROOT_DIR/target/AAVMF_VARS.fd" \
  -drive format=raw,file="fat:rw:$ROOT_DIR/target/uefi-esp" &
QEMU_PID=$!

cleanup() {
  if kill -0 "$QEMU_PID" >/dev/null 2>&1; then
    kill "$QEMU_PID" >/dev/null 2>&1 || true
    wait "$QEMU_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# Wait for the playit TCP socket to accept connections (up to 20 seconds)
echo "Waiting for playit socket on port $PLAYIT_PORT..."
for _ in $(seq 1 100); do
  if nc -z 127.0.0.1 "$PLAYIT_PORT" 2>/dev/null; then
    echo "Playit socket ready."
    break
  fi
  sleep 0.2
done

if ! nc -z 127.0.0.1 "$PLAYIT_PORT" 2>/dev/null; then
  echo "Playit socket never became ready on port $PLAYIT_PORT." >&2
  exit 1
fi

# Give the UEFI app a moment to finish initialization
sleep 2

# Run Node.js tests
echo "Running UEFI playit tests..."
RLVGL_UEFI_PLAYIT_PORT="$PLAYIT_PORT" \
  node --test "$ROOT_DIR/playit/node/test/uefi-disco.test.js"

echo "UEFI playit tests passed."
