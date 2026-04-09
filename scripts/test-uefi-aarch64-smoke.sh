#!/usr/bin/env bash
#
# Headless smoke test for the AArch64 UEFI disco demo. Boots under QEMU,
# captures one framebuffer image before input, injects Enter, captures again,
# and verifies that the rendered output changes.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MONITOR_SOCK="${MONITOR_SOCK:-/tmp/rlvgl-uefi-monitor.sock}"
BOOT_SHOT="${BOOT_SHOT:-/tmp/rlvgl-uefi-boot.ppm}"
INPUT_SHOT="${INPUT_SHOT:-/tmp/rlvgl-uefi-input.ppm}"
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

rm -f "$MONITOR_SOCK" "$BOOT_SHOT" "$INPUT_SHOT"

cd "$ROOT_DIR"
mkdir -p target/uefi-esp/EFI/BOOT
cp "$UEFI_VARS" target/AAVMF_VARS.fd
cargo build --bin rlvgl-uefi-disco --target "$TARGET_TRIPLE" --features uefi
cp "target/$TARGET_TRIPLE/debug/rlvgl-uefi-disco.efi" target/uefi-esp/EFI/BOOT/BOOTAA64.EFI

"$QEMU_BIN" \
  -machine virt \
  -cpu cortex-a72 \
  -m 512 \
  -display none \
  -serial none \
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

for _ in $(seq 1 50); do
  [[ -S "$MONITOR_SOCK" ]] && break
  sleep 0.2
done

if [[ ! -S "$MONITOR_SOCK" ]]; then
  echo "QEMU monitor socket was not created: $MONITOR_SOCK" >&2
  exit 1
fi

send_monitor() {
  local command="$1"
  printf '%s\n' "$command" | nc -U "$MONITOR_SOCK" >/dev/null
}

sleep 4
send_monitor "screendump $BOOT_SHOT"
sleep 1
send_monitor "sendkey ret"
sleep 1
send_monitor "screendump $INPUT_SHOT"

if [[ ! -f "$BOOT_SHOT" || ! -f "$INPUT_SHOT" ]]; then
  echo "Expected screendumps were not created." >&2
  exit 1
fi

BOOT_HASH="$(shasum -a 256 "$BOOT_SHOT" | awk '{print $1}')"
INPUT_HASH="$(shasum -a 256 "$INPUT_SHOT" | awk '{print $1}')"

echo "Boot screenshot:  $BOOT_SHOT"
echo "Input screenshot: $INPUT_SHOT"
echo "Boot hash:        $BOOT_HASH"
echo "Input hash:       $INPUT_HASH"

if [[ "$BOOT_HASH" == "$INPUT_HASH" ]]; then
  echo "Smoke test failed: framebuffer did not change after Enter key injection." >&2
  exit 1
fi

echo "Smoke test passed: framebuffer changed after Enter key injection."
