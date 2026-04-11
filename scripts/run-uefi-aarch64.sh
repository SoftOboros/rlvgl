#!/usr/bin/env bash
#
# Build and boot the shared 747-style UEFI disco demo under QEMU on Apple
# Silicon hosts. The script intentionally keeps firmware discovery in
# environment variables so local package-manager paths stay out of the repo.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="${TARGET_TRIPLE:-aarch64-unknown-uefi}"
QEMU_BIN="${QEMU_BIN:-qemu-system-aarch64}"
UEFI_CODE="${UEFI_CODE:-}"
UEFI_VARS="${UEFI_VARS:-}"
ESP_DIR="${ESP_DIR:-$ROOT_DIR/target/uefi-esp}"
VARS_COPY="${VARS_COPY:-$ROOT_DIR/target/AAVMF_VARS.fd}"
PLAYIT_PORT="${PLAYIT_PORT:-4567}"
QEMU_EXTRA_ARGS="${QEMU_EXTRA_ARGS:-}"

if [[ -z "$UEFI_CODE" ]]; then
  for candidate in \
    /opt/homebrew/share/qemu/edk2-aarch64-code.fd \
    /usr/local/share/qemu/edk2-aarch64-code.fd
  do
    if [[ -f "$candidate" ]]; then
      UEFI_CODE="$candidate"
      break
    fi
  done
fi

if [[ -z "$UEFI_VARS" ]]; then
  for candidate in \
    /opt/homebrew/share/qemu/edk2-arm-vars.fd \
    /usr/local/share/qemu/edk2-arm-vars.fd
  do
    if [[ -f "$candidate" ]]; then
      UEFI_VARS="$candidate"
      break
    fi
  done
fi

if ! command -v "$QEMU_BIN" >/dev/null 2>&1; then
  echo "Missing QEMU binary: $QEMU_BIN" >&2
  echo "Install qemu-system-aarch64 or set QEMU_BIN to the correct path." >&2
  exit 1
fi

if [[ -z "$UEFI_CODE" || -z "$UEFI_VARS" ]]; then
  echo "Set UEFI_CODE and UEFI_VARS to your AArch64 UEFI firmware images." >&2
  echo "Example:" >&2
  echo "  export UEFI_CODE=/path/to/AAVMF_CODE.fd" >&2
  echo "  export UEFI_VARS=/path/to/AAVMF_VARS.fd" >&2
  exit 1
fi

if [[ ! -f "$UEFI_CODE" ]]; then
  echo "UEFI_CODE does not exist: $UEFI_CODE" >&2
  exit 1
fi

if [[ ! -f "$UEFI_VARS" ]]; then
  echo "UEFI_VARS does not exist: $UEFI_VARS" >&2
  exit 1
fi

env rustup target list --installed | grep -qx "$TARGET_TRIPLE" || {
  echo "Rust target $TARGET_TRIPLE is not installed." >&2
  echo "Run: rustup target add $TARGET_TRIPLE" >&2
  exit 1
}

rm -rf "$ESP_DIR"
mkdir -p "$ESP_DIR/EFI/BOOT"
cp "$UEFI_VARS" "$VARS_COPY"

cd "$ROOT_DIR"
cargo build --manifest-path "$ROOT_DIR/examples/uefi-disco/Cargo.toml" --target "$TARGET_TRIPLE"
cp "examples/uefi-disco/target/$TARGET_TRIPLE/debug/rlvgl-uefi-disco.efi" "$ESP_DIR/EFI/BOOT/BOOTAA64.EFI"

echo "Playit automation socket: tcp://127.0.0.1:$PLAYIT_PORT"

#
# Input devices:
#   `-machine virt` ships without USB, so EDK2's UsbKbDxe has no
#   keyboard to enumerate and ConIn sees nothing from the GUI window.
#   Adding qemu-xhci + usb-kbd gives EDK2 a real HID keyboard whose
#   arrow / escape / function keys flow through ConIn and into the
#   `ConsoleTransport` special-key translator in main.rs. We
#   intentionally do NOT attach a usb-mouse: `DiscoCapabilities::uefi`
#   has `pointer: false`, and omitting the mouse stops QEMU's display
#   backend from grabbing the host cursor.
exec "$QEMU_BIN" \
  -machine virt \
  -cpu cortex-a72 \
  -m 512 \
  -serial stdio \
  -display default \
  -device ramfb \
  -device qemu-xhci,id=xhci \
  -device usb-kbd,bus=xhci.0 \
  -drive if=pflash,format=raw,readonly=on,file="$UEFI_CODE" \
  -drive if=pflash,format=raw,file="$VARS_COPY" \
  -drive format=raw,file="fat:rw:$ESP_DIR" \
  ${QEMU_EXTRA_ARGS}
