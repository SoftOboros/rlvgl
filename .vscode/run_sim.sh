#!/bin/bash
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
  TARGET="aarch64-apple-darwin"
else
  TARGET="x86_64-apple-darwin"
fi

exec "${0%/*}/../target/$TARGET/debug/rlvgl-sim" "$@"
