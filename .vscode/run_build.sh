#!/bin/bash
set -e

ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
  TARGET="aarch64-apple-darwin"
else
  TARGET="x86_64-apple-darwin"
fi

echo "Building for target: $TARGET"
cargo build --target "$TARGET" "$@"
