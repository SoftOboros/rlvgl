#!/usr/bin/env bash
# scripts/osx-rust-install.sh - Provision macOS toolchain and environment for rlvgl builds.

set -euo pipefail

if [[ "${TRACE:-0}" == "1" ]]; then
  set -x
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "[error] This installer only supports macOS hosts." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ENV_REPORT=()
PATH_REPORT=()
PKGCFG_REPORT=()

log_env() {
  ENV_REPORT+=("$1")
}

log_path() {
  PATH_REPORT+=("$1")
}

log_pkgcfg() {
  PKGCFG_REPORT+=("$1")
}

require_homebrew() {
  if ! command -v brew >/dev/null 2>&1; then
    cat >&2 <<'MSG'
[error] Homebrew is required but was not found on PATH.
Install Homebrew from https://brew.sh/ and rerun this script.
MSG
    exit 1
  fi
}

install_formulae() {
  local formulas=(
    cmake
    ninja
    pkg-config
    sccache
    llvm
    libusb
    python@3.11
    sdl2
    freetype
    rustup-init
  )

  echo "[info] Ensuring Homebrew formulas are installed..."
  for formula in "${formulas[@]}"; do
    if brew list --versions "$formula" >/dev/null 2>&1; then
      echo "  - $formula already installed"
    else
      echo "  - installing $formula"
      brew install "$formula"
    fi
  done
}

install_rust_toolchains() {
  echo "[info] Bootstrapping Rust toolchains..."
  if ! command -v rustup >/dev/null 2>&1; then
    echo "  - installing rustup"
    rustup-init -y --no-modify-path --profile minimal --default-toolchain stable
  else
    echo "  - rustup already installed"
    if ! rustup self update >/dev/null 2>&1; then
      echo "    warning: rustup self-update not available (skipping)"
    fi
  fi

  # shellcheck source=/dev/null
  if [[ -f "${HOME}/.cargo/env" ]]; then
    source "${HOME}/.cargo/env"
  fi

  rustup toolchain install stable --profile minimal
  rustup toolchain install nightly --profile minimal

  rustup default stable

  rustup component add --toolchain stable rustfmt clippy rust-src rust-analyzer llvm-tools-preview
  rustup component add --toolchain nightly rustfmt clippy rust-src rust-analyzer llvm-tools-preview

  rustup target add --toolchain stable thumbv7em-none-eabihf

  if ! cargo install --list | grep -q '^cargo-binutils '; then
    cargo install --locked cargo-binutils
  fi

  if ! cargo install --list | grep -q '^grcov '; then
    cargo install --locked grcov
  fi

  if ! command -v probe-rs >/dev/null 2>&1; then
    if command -v curl >/dev/null 2>&1; then
      echo "  - installing probe-rs tools"
      curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
    else
      echo "  - skipping probe-rs tools (curl not found)"
    fi
  else
    echo "  - probe-rs already installed"
  fi
}

ensure_env_var() {
  local name="$1"
  local value="$2"
  local description="$3"

  if [[ -n "${!name:-}" ]]; then
    log_env "Using existing $name=${!name} (${description})"
  else
    export "$name=$value"
    log_env "Set $name=$value (${description})"
  fi
}

ensure_path_prefix() {
  local prefix="$1"
  local label="$2"

  if [[ -d "$prefix" ]]; then
    if [[ ":$PATH:" != *":$prefix:"* ]]; then
      export PATH="$prefix:$PATH"
      log_path "Prepended $prefix to PATH (${label})"
    else
      log_path "PATH already includes $prefix (${label})"
    fi
  else
    log_path "Skipped PATH update for ${label} (missing $prefix)"
  fi
}

ensure_pkg_config() {
  local dir="$1"
  local label="$2"

  if [[ -d "$dir" ]]; then
    if [[ -z "${PKG_CONFIG_PATH:-}" ]]; then
      export PKG_CONFIG_PATH="$dir"
      log_pkgcfg "Initialized PKG_CONFIG_PATH with $dir (${label})"
    elif [[ ":${PKG_CONFIG_PATH}:" != *":${dir}:"* ]]; then
      export PKG_CONFIG_PATH="$dir:${PKG_CONFIG_PATH}"
      log_pkgcfg "Prepended $dir to PKG_CONFIG_PATH (${label})"
    else
      log_pkgcfg "PKG_CONFIG_PATH already includes $dir (${label})"
    fi
  else
    log_pkgcfg "Skipped PKG_CONFIG_PATH update for ${label} (missing $dir)"
  fi
}

summarize_environment() {
  echo
  echo "[info] Environment variable status:"
  for entry in "${ENV_REPORT[@]}"; do
    echo "  - ${entry}"
  done

  echo
  echo "[info] PATH updates:"
  for entry in "${PATH_REPORT[@]}"; do
    echo "  - ${entry}"
  done

  echo
  echo "[info] PKG_CONFIG_PATH updates:"
  for entry in "${PKGCFG_REPORT[@]}"; do
    echo "  - ${entry}"
  done

  cat <<'NEXT'

[hint] To persist these settings across terminal sessions, append the exports above to your shell profile (e.g. ~/.zshrc) or source this script.
NEXT
}

main() {
  require_homebrew
  brew update
  install_formulae
  brew cleanup --prune=all >/dev/null 2>&1 || true

  install_rust_toolchains

  local llvm_prefix="$(brew --prefix llvm 2>/dev/null || true)"
  local rlottie_prefix="$(brew --prefix rlottie 2>/dev/null || true)"

  ensure_env_var "CARGO_HOME" "${HOME}/.cargo" "Cargo home directory"
  ensure_env_var "RUSTUP_HOME" "${HOME}/.rustup" "Rustup home directory"
  ensure_env_var "RLVGL_LINKER_SCRIPT" "${REPO_ROOT}/memory.x" "Linker script for thumbv7em-none-eabihf builds"
  ensure_env_var "CARGO_INCREMENTAL" "0" "Disable incremental compilation per project policy"

  ensure_path_prefix "${CARGO_HOME:-$HOME/.cargo}/bin" "Cargo binaries"

  if [[ -n "${llvm_prefix}" ]]; then
    ensure_path_prefix "${llvm_prefix}/bin" "Homebrew LLVM"
  else
    log_path "LLVM prefix unavailable; PATH unchanged for LLVM"
  fi

  if [[ -n "${rlottie_prefix}" ]]; then
    ensure_pkg_config "${rlottie_prefix}/lib/pkgconfig" "rlottie"
  else
    log_pkgcfg "rlottie prefix unavailable; PKG_CONFIG_PATH unchanged"
  fi

  summarize_environment

  echo
  echo "[done] macOS toolchain setup for rlvgl complete."
}

main "$@"
