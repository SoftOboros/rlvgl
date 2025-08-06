#!/usr/bin/env bash
set -euo pipefail

BASE=${1:-origin/main}

changed=()

if git diff --name-only "$BASE" HEAD | grep -q '^core/'; then
  changed+=("rlvgl-core")
fi
if git diff --name-only "$BASE" HEAD | grep -q '^widgets/'; then
  changed+=("rlvgl-widgets")
fi
if git diff --name-only "$BASE" HEAD | grep -q '^platform/'; then
  changed+=("rlvgl-platform")
fi
if git diff --name-only "$BASE" HEAD | grep -q -e '^src/' -e '^Cargo.toml'; then
  changed+=("rlvgl")
fi

for crate in "${changed[@]}"; do
  echo "Publishing $crate"
  cargo publish -p "$crate" --token "$CARGO_REGISTRY_TOKEN" --no-verify || echo "⚠️ publish $crate failed."
done
