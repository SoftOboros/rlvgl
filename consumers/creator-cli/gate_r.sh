#!/usr/bin/env bash
# CRATES-CI-05 — Gate R runner for the creator-cli Consumer (CRATES-CI-00
# §3 "Gate R", §5 gate topology, §12 CRATES-CI-05 acceptance).
#
# Gate R for the creator binary is NOT smoke.sh-against-a-staged-package:
# it is the literal end-user install path — `cargo install rlvgl
# --features creator` resolved from real crates.io — followed by the same
# CLI round-trip smoke.sh runs (init -> scan -> license group -> convert
# -> sync, mirroring .github/workflows/creator-e2e.yml's verb sequence).
#
# --locked vs unlocked: the packaged root crate DOES ship its Cargo.lock
# (root Cargo.toml `include` lists "/Cargo.lock"), so `--locked` would
# work — but users type plain `cargo install rlvgl --features creator`,
# and Gate R exists to verify the path users actually hit. Install
# unlocked, on purpose.
#
# Env:
#   GATE_R_FEATURES  feature list passed to cargo install
#                    (default: "creator"). Do NOT default this to
#                    creator_ui[,creator_ui_automation] until a version
#                    that builds them (0.2.2+) is the latest published
#                    0.2.x — 0.2.1's simulator path is known-broken
#                    (CRATES-CI-00 §15, P-INCLUDE).
#
# Failure reporting: the EXIT trap names the failing phase so a red run
# distinguishes "crates.io install failed" (registry truth diverged:
# yanked dep, broken published version, index lag) from "installed binary
# misbehaves" (round-trip / output-assertion failure).
#
# macOS bash 3.2 portable: no mapfile, no associative arrays.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
GATE_R_FEATURES="${GATE_R_FEATURES:-creator}"

INSTALL_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/rlvgl-creator-gate-r-install.XXXXXX")"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/rlvgl-creator-gate-r-work.XXXXXX")"

PHASE="install (cargo install rlvgl from crates.io)"
on_exit() {
  status=$?
  rm -rf "$INSTALL_ROOT" "$WORK"
  if [ "$status" -ne 0 ]; then
    echo "gate_r.sh: FAIL — phase: $PHASE" >&2
  fi
  exit "$status"
}
trap on_exit EXIT

# --- 1. Install the creator binary from real crates.io ------------------------
echo "gate_r.sh: cargo install rlvgl --features $GATE_R_FEATURES (registry truth)"
cargo install rlvgl --features "$GATE_R_FEATURES" --root "$INSTALL_ROOT"

# Report the version crates.io actually resolved (cargo install records it
# in <root>/.crates.toml as: "rlvgl <version> (registry+...)" = [...]).
RESOLVED="$(grep -o '"rlvgl [0-9][^ ]*' "$INSTALL_ROOT/.crates.toml" | head -n 1 | cut -d' ' -f2 || true)"
echo "gate_r.sh: installed rlvgl version: ${RESOLVED:-unknown}"

BIN="$INSTALL_ROOT/bin/rlvgl-creator"
if [ ! -x "$BIN" ]; then
  echo "gate_r.sh: installed binary not found at $BIN" >&2
  exit 1
fi

# --- 2. CLI round-trip in a temp dir ------------------------------------------
# Same sequence and assertions as smoke.sh's temp-dir section: mirrors
# creator-e2e.yml's pipeline (scan/convert/sync) minimally, preceded by
# `init` since we start from an empty dir. The PNG fixture is copied from
# the repo at run time (no committed binary fixture).
PHASE="round-trip (init/scan/convert/sync with installed binary)"
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

# --- 3. Assert expected outputs -----------------------------------------------
PHASE="output assertions (round-trip artifacts)"
fail=0
for f in \
  manifest.yml \
  icons/rlvgl-logo.raw \
  out/features.toml \
  out/rlvgl_index.rs; do
  if [ ! -f "$WORK/$f" ]; then
    echo "gate_r.sh: FAIL — missing expected output: $f" >&2
    fail=1
  fi
done
if ! grep -q 'rlvgl-logo' "$WORK/manifest.yml"; then
  echo "gate_r.sh: FAIL — manifest.yml does not list the scanned asset" >&2
  fail=1
fi
if [ "$fail" -ne 0 ]; then
  exit 1
fi

echo "gate_r.sh: PASS — creator CLI round-trip from crates.io install (rlvgl ${RESOLVED:-unknown}, features: $GATE_R_FEATURES)"
