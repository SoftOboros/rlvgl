#!/usr/bin/env bash
set -euo pipefail

# normalize target arg
TARGET="${1:-h7}"
TARGET="$(echo "$TARGET" | tr '[:upper:]' '[:lower:]')"

# map shorthand → actual gdb init file
case "$TARGET" in
  h7|stm32h7*) INIT="gdb_init_h7.gdb" ;;
  f4|stm32f4*) INIT="gdb_init_f4.gdb" ;;
  f1|stm32f1*) INIT="gdb_init_f1.gdb" ;;
  *) echo "Unknown target '$TARGET' (defaulting to h7)" ; INIT="gdb_init_h7.gdb" ;;
esac

# pick a GDB binary, preferring VS Code's cortex-debug.gdbPath if configured
pick_gdb() {
  # 1) respect explicit env override
  if [ -n "${GDB_BIN:-}" ] && [ -x "$GDB_BIN" ]; then
    echo "$GDB_BIN"
    return 0
  fi

  # 2) try VS Code setting
  if [ -f ".vscode/settings.json" ]; then
    local VS_GDB
    VS_GDB=$(grep -oE '"cortex-debug\.gdbPath"\s*:\s*"[^"]+"' .vscode/settings.json | sed -E 's/.*:\s*"(.*)"/\1/') || true
    if [ -n "${VS_GDB:-}" ] && [ -x "$VS_GDB" ]; then
      echo "$VS_GDB"
      return 0
    fi
  fi

  # 3) common fallbacks on PATH
  for cand in arm-none-eabi-gdb gdb-multiarch gdb; do
    if command -v "$cand" >/dev/null 2>&1; then
      echo "$(command -v "$cand")"
      return 0
    fi
  done

  echo ""  # not found
}

GDB_BIN_RESOLVED=$(pick_gdb)
if [ -z "$GDB_BIN_RESOLVED" ]; then
  echo "[gdb] error: no suitable GDB found (env GDB_BIN, VS Code setting, or PATH)" >&2
  exit 1
fi

echo "[gdb] using: $GDB_BIN_RESOLVED"

# optional ELF path for symbols/program
ELF_ARG="${2:-}"
ELF_RESOLVED=""
if [ -n "$ELF_ARG" ]; then
  if [ -f "$ELF_ARG" ]; then
    ELF_RESOLVED="$ELF_ARG"
  else
    echo "[gdb] warning: ELF not found: $ELF_ARG" >&2
  fi
else
  # try a reasonable default by target
  case "$TARGET" in
    h7|stm32h7*) DEF_ELF="target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-disco" ;;
    *) DEF_ELF="" ;;
  esac
  if [ -n "${DEF_ELF:-}" ] && [ -f "$DEF_ELF" ]; then
    ELF_RESOLVED="$DEF_ELF"
  fi
fi

if [ -n "$ELF_RESOLVED" ]; then
  echo "[gdb] using ELF: $ELF_RESOLVED"
fi

while :; do
  echo "[gdb] launching with $INIT"
  if [ -n "$ELF_RESOLVED" ]; then
    "$GDB_BIN_RESOLVED" -q "$ELF_RESOLVED" --command="$INIT" || true
  else
    "$GDB_BIN_RESOLVED" -q --command="$INIT" || true
  fi
  echo "[gdb] disconnected; retrying in 1s"
  sleep 1
done
