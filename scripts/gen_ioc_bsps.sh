#!/usr/bin/env bash
# gen_ioc_bsps.sh - Generate board support package names from STM32CubeMX IOC files.
#
# This script scans the STM32 Open Pin Data board definitions and prints a
# normalized slug for each board. It handles variant suffixes such as
# TrustZone-enabled or BareMetal configurations to keep the generated names
# unique.

set -euo pipefail

# Directory containing *.ioc files. Allow override via first argument.
ROOT_DIR=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null || printf '.')
DATA_DIR=${1:-"$ROOT_DIR/../STM32_open_pin_data/boards"}
if [[ ! -d "$DATA_DIR" ]]; then
    echo "Usage: $0 <path-to-STM32_open_pin_data/boards>" >&2
    exit 1
fi

variant_suffix() {
    case "$1" in
        TrustZoneEnabled) echo "tz" ;;
        FullSecure) echo "fs" ;;
        BareMetalEnabled) echo "bme" ;;
        M33TDCID) echo "m33tdcid" ;;
        MultiToSingleCore) echo "mtsc" ;;
        *) echo "${1,,}" ;;
    esac
}

shopt -s nullglob
files=("$DATA_DIR"/*.ioc)
total=${#files[@]}
printf 'Generating BSPs for %d boards...\n' "$total"

idx=1
for file in "${files[@]}"; do
    base=${file##*/}
    base=${base%.ioc}
    base=${base%_Board_AllConfig}
    prefix=${base%_STM32*}
    after=${base##*_STM32}
    name=${prefix##*_}
    name=${name,,}
    name=${name//-/_}
    if [[ "$after" == *_* ]]; then
        variant=${after#*_}
        suffix=$(variant_suffix "$variant")
        name="${name}_${suffix}"
    fi
    # Placeholder for BSP generation logic per board if needed.
    printf '[%d/%d] %s\n' "$idx" "$total" "$name"
    echo '    done'
    ((idx++))
done

