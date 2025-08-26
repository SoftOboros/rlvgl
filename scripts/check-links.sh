#!/usr/bin/env bash
# scripts/check-links.sh - Validate markdown links using lychee.

set -euo pipefail

if ! command -v lychee >/dev/null 2>&1; then
  echo "lychee not installed; install with 'cargo install lychee'" >&2
  exit 1
fi

find . -name '*.md' -not -path './lvgl/*' |
  xargs lychee --no-progress
