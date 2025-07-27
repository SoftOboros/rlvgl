#!/usr/bin/env bash
# Git pre-commit hook to enforce formatting and linting
set -e

cargo fmt --all
cargo clippy --workspace -- -D warnings
