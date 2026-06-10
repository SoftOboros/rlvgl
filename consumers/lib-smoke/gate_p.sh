#!/usr/bin/env bash
# CRATES-CI-01 — Gate P/R runner for the lib-smoke Consumer Project.
# Governed by docs/crates-ci/CRATES-CI-00-CONCEPTS.md §8 (Consumer Project
# contract) and §9 INV-C2 (no path/[patch] routes into the workspace; the
# extracted package dirs referenced by $STAGE_DIR/patch-config.toml are
# allowed — they are the packaged file set, not the workspace).
#
# Modes:
#   (default / GATE=p)  Gate P: consume the Staged Registry produced by
#                       scripts/crates_ci_stage.sh via [patch.crates-io].
#   GATE=r              Gate R: remove the patch config and build against
#                       real crates.io (post-publish registry truth).
#
# Env:
#   STAGE_DIR  staging root (default: <repo>/target/crates-ci, i.e.
#              ../../target/crates-ci relative to this script)
#   GATE       p (default) or r
#
# macOS bash 3.2 portable: no mapfile, no associative arrays.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

STAGE_DIR="${STAGE_DIR:-$SCRIPT_DIR/../../target/crates-ci}"
GATE="${GATE:-p}"

if [ "$GATE" = "r" ]; then
  # Gate R: registry truth — no patch table may shadow crates.io.
  if [ -f "$SCRIPT_DIR/.cargo/config.toml" ]; then
    rm -f "$SCRIPT_DIR/.cargo/config.toml"
    echo "gate_p.sh: GATE=r — removed .cargo/config.toml (building against crates.io)"
  fi
else
  PATCH_CONFIG="$STAGE_DIR/patch-config.toml"
  if [ ! -f "$PATCH_CONFIG" ]; then
    echo "gate_p.sh: $PATCH_CONFIG not found." >&2
    echo "gate_p.sh: run scripts/crates_ci_stage.sh first (Gate P staging)." >&2
    exit 1
  fi
  mkdir -p "$SCRIPT_DIR/.cargo"
  cp "$PATCH_CONFIG" "$SCRIPT_DIR/.cargo/config.toml"
  echo "gate_p.sh: staged patch table installed from $PATCH_CONFIG"
  # A stale Cargo.lock pins registry sources and cargo will honor it,
  # leaving every patch entry [[patch.unused]] — the gate would then
  # "pass" without touching the staged crates (observed during
  # CRATES-CI-01 bring-up). Always resolve fresh under Gate P.
  rm -f "$SCRIPT_DIR/Cargo.lock"
fi

cargo build

if [ "$GATE" != "r" ]; then
  # Gate honesty assertion: every rlvgl-* package in the resolved graph
  # MUST come from the staged package set, not the registry. A single
  # registry-sourced rlvgl crate means the gate is not testing what it
  # claims to test. ([[patch.unused]] entries are expected and benign —
  # the patch table carries all staged crates, and this consumer only
  # depends on a subset.)
  if awk '
    /^name = "rlvgl/ { rl = 1; next }
    /^name = /       { rl = 0; next }
    rl && /^source = "registry/ { bad = 1 }
    END { exit bad }
  ' "$SCRIPT_DIR/Cargo.lock"; then
    :
  else
    echo "gate_p.sh: FAIL — an rlvgl-* crate resolved from the registry under GATE=p:" >&2
    grep -B1 'source = "registry' "$SCRIPT_DIR/Cargo.lock" | grep 'name = "rlvgl' >&2
    exit 1
  fi
  echo "gate_p.sh: staged-source assertion passed (no registry rlvgl-* crates)"
fi

OUT="$(cargo run 2>/dev/null | tail -n 1)"
echo "gate_p.sh: consumer output: $OUT"
if ! echo "$OUT" | grep -q "OK"; then
  echo "gate_p.sh: FAIL — expected OK from crates-ci-lib-smoke" >&2
  exit 1
fi
echo "gate_p.sh: PASS (GATE=$GATE)"
