#!/usr/bin/env bash
set -euo pipefail

BASE=${1:-origin/main}
DRY_RUN=${DRY_RUN:-0}
INDEX_WAIT_SECONDS=${INDEX_WAIT_SECONDS:-30}

changed=()

append_unique() {
  local item="$1"
  local existing
  for existing in "${changed[@]}"; do
    if [[ "$existing" == "$item" ]]; then
      return
    fi
  done
  changed+=("$item")
}

path_changed() {
  local pattern="$1"
  grep -qE "$pattern" <<<"$DIFF_FILES"
}

if ! git rev-parse --verify "${BASE}^{commit}" >/dev/null 2>&1; then
  echo "Base commit not found: $BASE" >&2
  exit 1
fi

BASE_SHA=$(git rev-parse "$BASE")
HEAD_SHA=$(git rev-parse HEAD)
BASE_DESC=$(git describe --tags --always "$BASE_SHA" 2>/dev/null || echo "$BASE_SHA")
HEAD_DESC=$(git describe --tags --always "$HEAD_SHA" 2>/dev/null || echo "$HEAD_SHA")
DIFF_FILES=$(git diff --name-only "$BASE_SHA" "$HEAD_SHA")

echo "Publish diff:"
echo "  base: $BASE_SHA ($BASE_DESC)"
echo "  head: $HEAD_SHA ($HEAD_DESC)"

if [[ -z "$DIFF_FILES" ]]; then
  echo "No files changed between base and head."
  echo "No changed crates detected; nothing to publish."
  exit 0
fi

chipdb_crates=(
  rlvgl-chips-stm
  rlvgl-chips-nrf
  rlvgl-chips-esp
  rlvgl-chips-nxp
  rlvgl-chips-silabs
  rlvgl-chips-microchip
  rlvgl-chips-renesas
  rlvgl-chips-ti
  rlvgl-chips-rp2040
)

for crate in "${chipdb_crates[@]}"; do
  if path_changed "^chipdb/${crate}/"; then
    append_unique "$crate"
  fi
done

if path_changed '^chips/stm/bsps/'; then
  append_unique "rlvgl-bsps-stm"
fi
if path_changed '^core/'; then
  append_unique "rlvgl-core"
fi
if path_changed '^widgets/'; then
  append_unique "rlvgl-widgets"
fi
if path_changed '^ui/'; then
  append_unique "rlvgl-ui"
fi
if path_changed '^platform/'; then
  append_unique "rlvgl-platform"
fi
if path_changed '^i18n/'; then
  append_unique "rlvgl-i18n"
fi
if path_changed '^examples/apps/demo/'; then
  append_unique "rlvgl-app-demo"
fi
if path_changed '^(src/|Cargo\.toml$|build\.rs$|README\.md$|rlvgl-logo\.png$|examples/|tests/)'; then
  append_unique "rlvgl"
fi

if [[ ${#changed[@]} -eq 0 ]]; then
  echo "No changed crates detected; nothing to publish."
  exit 0
fi

echo "Changed crates (publish order):"
for crate in "${changed[@]}"; do
  echo "  - $crate"
done

if [[ "$DRY_RUN" == "1" ]]; then
  echo "Dry run enabled; skipping cargo publish."
  exit 0
fi

if [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
  echo "CARGO_REGISTRY_TOKEN is not set." >&2
  exit 1
fi

prev=""
for crate in "${changed[@]}"; do
  # crates.io needs time to index a new publish before dependents can resolve it.
  if [[ -n "$prev" ]]; then
    echo "Waiting ${INDEX_WAIT_SECONDS}s for crates.io to index $prev..."
    sleep "$INDEX_WAIT_SECONDS"
  fi
  echo "Publishing $crate"
  if [[ "$crate" == "rlvgl-chips-stm" ]]; then
    scripts/stm32_afdb_pipeline.sh
    # The packaged chip database archive is generated during publish and is
    # intentionally gitignored in-tree, so force-add it for cargo packaging.
    git add -f chipdb/rlvgl-chips-stm/assets/chipdb.bin.zst
    cargo publish -p "$crate" --no-verify --allow-dirty
  elif [[ "$crate" == "rlvgl-bsps-stm" ]]; then
    scripts/gen_ioc_bsps.sh
    cargo publish -p "$crate" --no-verify --allow-dirty
  else
    cargo publish -p "$crate" --no-verify
  fi
  prev="$crate"
done
