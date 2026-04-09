#!/usr/bin/env bash
# scripts/kill-openocd.sh - Convenience helper to free OpenOCD standard ports.
#
# Purpose: Terminates any OpenOCD process bound to TCP ports 3333/4444/6666
# so VS Code's DISCO target can launch OpenOCD on the standard ports.
#
# Usage:
#   ./scripts/kill-openocd.sh          # kill anything on 3333/4444/6666
#   ./scripts/kill-openocd.sh 3333     # kill anything on a specific port
#
set -euo pipefail

ports=("${@:-3333 4444 6666}")

for p in ${ports[@]}; do
  if lsof -ti tcp:"$p" >/dev/null 2>&1; then
    echo "Killing process on port $p"
    kill -9 $(lsof -ti tcp:"$p") || true
  else
    echo "No process bound to port $p"
  fi
done

if pgrep -f openocd >/dev/null 2>&1; then
  echo "OpenOCD processes still running (not on specified ports):"
  pgrep -af openocd || true
fi

echo "Done."

