#!/usr/bin/env bash
# Install prerequisites and select the accepted dated CI qualification profile.
set -euo pipefail

git submodule update --init --recursive

# The container's floating nightly is not the qualification compiler. Preserve
# its other tools while installing the exact PCDN-13 profile and required target.
qualification_toolchain="nightly-2025-07-31"
if [[ -n "${RUSTUP_TOOLCHAIN:-}" && "$RUSTUP_TOOLCHAIN" != "$qualification_toolchain" ]]; then
    echo "Unexpected CI qualification toolchain: $RUSTUP_TOOLCHAIN" >&2
    exit 1
fi
if [[ "${GITHUB_ACTIONS:-}" == "true" && -z "${GITHUB_ENV:-}" ]]; then
    echo "GitHub Actions requires GITHUB_ENV to retain the qualification profile." >&2
    exit 1
fi
export RUSTUP_TOOLCHAIN="$qualification_toolchain"

rustup toolchain install "$qualification_toolchain" --profile minimal \
    --component rustfmt,clippy,rust-src,llvm-tools-preview \
    --target thumbv7em-none-eabihf --no-self-update

# Verify the compiler actually selected by bare Cargo/Rust commands. An identity
# mismatch fails; never fall back to the image default or another nightly.
rustc_version="$(rustc --version)"
cargo_version="$(cargo --version)"
printf '%s\n' "$rustc_version" "$cargo_version"
if [[ "$rustc_version" != "rustc 1.90.0-nightly (3048886e5 "* ]]; then
    echo "Unexpected qualification rustc identity: $rustc_version" >&2
    exit 1
fi
if [[ "$cargo_version" != "cargo 1.90.0-nightly (9b296973b "* ]]; then
    echo "Unexpected qualification Cargo identity: $cargo_version" >&2
    exit 1
fi
rustc_verbose="$(rustc --version --verbose)"
printf '%s\n' "$rustc_verbose"
cargo --version --verbose
compiler_host="$(printf '%s\n' "$rustc_verbose" | sed -n 's/^host: //p')"
if [[ -z "$compiler_host" ]]; then
    echo "Qualification compiler host identity is missing." >&2
    exit 1
fi
installed_components="$(rustup component list --toolchain "$qualification_toolchain" --installed)"
printf '%s\n' "$installed_components"
# Rustup's genuine installed ID is llvm-tools-HOST; the install request above
# uses its llvm-tools-preview alias. Require the actual canonical component IDs.
for required_component in "rustfmt-$compiler_host" "clippy-$compiler_host" \
    rust-src "llvm-tools-$compiler_host"; do
    if ! printf '%s\n' "$installed_components" | grep -Fxq "$required_component"; then
        echo "Qualification component is not installed: $required_component" >&2
        exit 1
    fi
done
installed_targets="$(rustup target list --toolchain "$qualification_toolchain" --installed)"
printf '%s\n' "$installed_targets"
if ! printf '%s\n' "$installed_targets" | grep -Fxq thumbv7em-none-eabihf; then
    echo "Qualification ARM standard-library target is not installed." >&2
    exit 1
fi

# GitHub steps run in separate shells. Select the same dated compiler for later
# staging, consumer, firmware and docs steps without changing checkout sources
# or a user's/global rustup default. Other callers must select this profile in
# their enclosing shell; this script's export cannot escape a child process.
if [[ -n "${GITHUB_ENV:-}" ]]; then
    printf 'RUSTUP_TOOLCHAIN=%s\n' "$qualification_toolchain" >> "$GITHUB_ENV"
fi
