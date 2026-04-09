#!/usr/bin/env bash
set -euo pipefail

BASE=${1:-origin/main}
DRY_RUN=${DRY_RUN:-0}
INDEX_WAIT_SECONDS=${INDEX_WAIT_SECONDS:-30}
METADATA_JSON=$(mktemp)
VERSIONS_TSV=$(mktemp)

cleanup() {
  rm -f "$METADATA_JSON" "$VERSIONS_TSV"
}

trap cleanup EXIT

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

crate_version() {
  local crate="$1"
  awk -F '\t' -v crate="$crate" '$1 == crate { print $2; exit }' "$VERSIONS_TSV"
}

crate_version_exists() {
  local crate="$1"
  local version="$2"
  local status

  if command -v curl >/dev/null 2>&1; then
    status=$(curl -sS -o /dev/null -w '%{http_code}' \
      --connect-timeout 20 \
      --user-agent 'rlvgl-publish-script' \
      "https://crates.io/api/v1/crates/${crate}/${version}") || {
      echo "Failed to query crates.io for $crate v$version via curl." >&2
      return 2
    }
  else
    status=$(python3 - "$crate" "$version" <<'PY'
import sys
import urllib.error
import urllib.request

crate, version = sys.argv[1], sys.argv[2]
url = f"https://crates.io/api/v1/crates/{crate}/{version}"
request = urllib.request.Request(url, headers={"User-Agent": "rlvgl-publish-script"})

try:
    with urllib.request.urlopen(request, timeout=20) as response:
        print(response.status)
except urllib.error.HTTPError as exc:
    print(exc.code)
except Exception as exc:
    print(f"ERR:{exc}")
PY
)
  fi

  case "$status" in
    200)
      return 0
      ;;
    404)
      return 1
      ;;
    ERR:*)
      echo "Failed to query crates.io for $crate v$version: ${status#ERR:}" >&2
      return 2
      ;;
    *)
      echo "Unexpected crates.io response for $crate v$version: $status" >&2
      return 2
      ;;
  esac
}

publish_crate() {
  local crate="$1"
  local version="$2"
  shift 2

  local log_file
  local publish_status
  local retry_after

  log_file=$(mktemp)

  set +e
  "$@" 2>&1 | tee "$log_file"
  publish_status=${PIPESTATUS[0]}
  set -e

  if [[ "$publish_status" -eq 0 ]]; then
    rm -f "$log_file"
    return 0
  fi

  echo "Publish failed for $crate v$version." >&2
  if grep -q '429 Too Many Requests' "$log_file"; then
    retry_after=$(sed -n 's/.*Please try again after \(.* GMT\) and see.*/\1/p' "$log_file" | tail -n 1)
    if [[ -n "$retry_after" ]]; then
      echo "crates.io rate limit hit while publishing $crate v$version. Retry after $retry_after." >&2
    else
      echo "crates.io rate limit hit while publishing $crate v$version. Retry this workflow after the limit window resets." >&2
    fi
  fi

  rm -f "$log_file"
  return "$publish_status"
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

cargo metadata --no-deps --format-version 1 >"$METADATA_JSON"
python3 - "$METADATA_JSON" >"$VERSIONS_TSV" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    metadata = json.load(handle)

for package in metadata["packages"]:
    print(f"{package['name']}\t{package['version']}")
PY

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
if path_changed '^playit/'; then
  append_unique "rlvgl-playit"
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

prev_published=""
for crate in "${changed[@]}"; do
  version=$(crate_version "$crate")
  if [[ -z "$version" ]]; then
    echo "Could not determine workspace version for $crate." >&2
    exit 1
  fi

  if crate_version_exists "$crate" "$version"; then
    echo "Skipping $crate v$version (already published on crates.io)"
    continue
  else
    status=$?
    if [[ "$status" -ne 1 ]]; then
      exit "$status"
    fi
  fi

  # crates.io needs time to index a new publish before dependents can resolve it.
  if [[ -n "$prev_published" ]]; then
    echo "Waiting ${INDEX_WAIT_SECONDS}s for crates.io to index $prev_published..."
    sleep "$INDEX_WAIT_SECONDS"
  fi
  echo "Publishing $crate v$version"
  if [[ "$crate" == "rlvgl-chips-stm" ]]; then
    scripts/stm32_afdb_pipeline.sh
    # The packaged chip database archive is generated during publish and is
    # intentionally gitignored in-tree, so force-add it for cargo packaging.
    git add -f chipdb/rlvgl-chips-stm/assets/chipdb.bin.zst
    publish_crate "$crate" "$version" cargo publish -p "$crate" --no-verify --allow-dirty
  elif [[ "$crate" == "rlvgl-bsps-stm" ]]; then
    scripts/gen_ioc_bsps.sh
    publish_crate "$crate" "$version" cargo publish -p "$crate" --no-verify --allow-dirty
  else
    publish_crate "$crate" "$version" cargo publish -p "$crate" --no-verify
  fi
  prev_published="$crate"
done
