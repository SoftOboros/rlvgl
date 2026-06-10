#!/usr/bin/env bash
# CRATES-CI-01 — creator-cli Consumer harness (CRATES-CI-00 §8.4).
#
# Builds the rlvgl-creator binary INSIDE the staged root-crate package dir
# ($STAGE_DIR/staged/rlvgl-<version>/) — exactly what
# `cargo install rlvgl --features creator` would compile — then runs a
# minimal CLI round-trip (init -> scan -> convert -> sync, mirroring
# .github/workflows/creator-e2e.yml's verb sequence) in a temp dir.
#
# Phase 01 scope is the `creator` CLI feature set only; CRATES-CI-04 adds
# `creator,creator_ui` plus the Layer W playit handshake.
#
# Env:
#   STAGE_DIR  staging root (default: <repo>/target/crates-ci, i.e.
#              ../../target/crates-ci relative to this script)
#
# macOS bash 3.2 portable: no mapfile, no associative arrays.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
STAGE_DIR="${STAGE_DIR:-$REPO_ROOT/target/crates-ci}"

# --- 1. Locate the staged root-crate package ---------------------------------
VERSIONS_TSV="$STAGE_DIR/versions.tsv"
if [ ! -f "$VERSIONS_TSV" ]; then
  echo "smoke.sh: $VERSIONS_TSV not found." >&2
  echo "smoke.sh: run scripts/crates_ci_stage.sh first (Gate P staging)." >&2
  exit 1
fi

RLVGL_VERSION="$(awk -F '\t' '$1 == "rlvgl" { print $2; exit }' "$VERSIONS_TSV")"
if [ -z "$RLVGL_VERSION" ]; then
  echo "smoke.sh: could not find rlvgl in $VERSIONS_TSV" >&2
  exit 1
fi

PKG="$STAGE_DIR/staged/rlvgl-$RLVGL_VERSION"
if [ ! -d "$PKG" ]; then
  echo "smoke.sh: staged package dir not found: $PKG" >&2
  exit 1
fi
echo "smoke.sh: staged root crate: $PKG"

# --- 2. Patch table, minus the root crate itself ------------------------------
# A package must not patch itself: filter the `rlvgl = { ... }` entry out of
# the staged patch table before installing it as the package's cargo config.
PATCH_CONFIG="$STAGE_DIR/patch-config.toml"
if [ ! -f "$PATCH_CONFIG" ]; then
  echo "smoke.sh: $PATCH_CONFIG not found." >&2
  exit 1
fi
mkdir -p "$PKG/.cargo"
grep -v '^rlvgl = ' "$PATCH_CONFIG" >"$PKG/.cargo/config.toml"
echo "smoke.sh: wrote $PKG/.cargo/config.toml (rlvgl self-patch filtered)"

# --- 3. Build the creator binary from the packaged file set -------------------
# Bin name + required-features per the root Cargo.toml [[bin]] section
# (name = "rlvgl-creator", required-features = ["creator"]).
(cd "$PKG" && cargo build --bin rlvgl-creator --features creator)

# CRATES-CI-02a: the umbrella `simulator` feature pulls the demo app crates
# (rlvgl-app-demo / rlvgl-app-disco-demo) and is the path a downstream user
# hits first per docs/CUSTOM-SIMULATOR.md. It shipped broken in 0.2.1
# (P-INCLUDE: disco-demo icons referenced outside its crate root, so the
# packaged crate could never compile). Keep it covered from packaged crates.
(cd "$PKG" && cargo build --lib --features simulator)
BIN="$PKG/target/debug/rlvgl-creator"
if [ ! -x "$BIN" ]; then
  echo "smoke.sh: built binary not found at $BIN" >&2
  exit 1
fi

# Gate honesty assertion (mirrors consumers/lib-smoke/gate_p.sh): every
# rlvgl-* dependency in the resolved graph MUST come from the staged
# package set, not the registry — a stale/embedded Cargo.lock can
# otherwise pin registry sources and leave the patch table unused.
if awk '
  /^name = "rlvgl/ { rl = 1; next }
  /^name = /       { rl = 0; next }
  rl && /^source = "registry/ { bad = 1 }
  END { exit bad }
' "$PKG/Cargo.lock"; then
  echo "smoke.sh: staged-source assertion passed (no registry rlvgl-* crates)"
else
  echo "smoke.sh: FAIL — an rlvgl-* crate resolved from the registry:" >&2
  grep -B1 'source = "registry' "$PKG/Cargo.lock" | grep 'name = "rlvgl' >&2
  exit 1
fi

# --- 4. CLI round-trip in a temp dir ------------------------------------------
# Mirrors creator-e2e.yml's pipeline (scan/convert/sync; lines 26-28)
# minimally, preceded by `init` since we start from an empty dir. The PNG
# fixture is copied from the repo at run time (no committed binary fixture).
WORK="$(mktemp -d "${TMPDIR:-/tmp}/rlvgl-creator-cli-smoke.XXXXXX")"
cleanup() {
  rm -rf "$WORK"
}
trap cleanup EXIT

cd "$WORK"
"$BIN" --silent init
cp "$REPO_ROOT/rlvgl-logo.png" icons/rlvgl-logo.png
"$BIN" --silent scan .
# convert (and sync) end with check --fix, which requires license metadata
# (src/bin/creator/check.rs:67-79) and fills per-asset licenses from a
# manifest group that lists the asset's path with a `license:` value. Declare
# the group the way a user would edit manifest.yml — covering both the
# scanned .png path and the .raw path convert renames it to. (The
# creator-e2e fixture sidesteps the license gate only because its asset
# files don't exist, so scan drops every entry.)
awk '
  $0 == "groups: {}" {
    print "groups:"
    print "  icons:"
    print "    assets:"
    print "    - icons/rlvgl-logo.png"
    print "    - icons/rlvgl-logo.raw"
    print "    license: MIT"
    next
  }
  { print }
' manifest.yml >manifest.yml.tmp
mv manifest.yml.tmp manifest.yml
"$BIN" --silent convert .
"$BIN" --silent sync --out out

# --- 5. Assert expected outputs -----------------------------------------------
fail=0
for f in \
  manifest.yml \
  icons/rlvgl-logo.raw \
  out/features.toml \
  out/rlvgl_index.rs; do
  if [ ! -f "$WORK/$f" ]; then
    echo "smoke.sh: FAIL — missing expected output: $f" >&2
    fail=1
  fi
done
if ! grep -q 'rlvgl-logo' "$WORK/manifest.yml"; then
  echo "smoke.sh: FAIL — manifest.yml does not list the scanned asset" >&2
  fail=1
fi
if [ "$fail" -ne 0 ]; then
  exit 1
fi

echo "smoke.sh: PASS — creator CLI round-trip from packaged crates"
