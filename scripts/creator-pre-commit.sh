#!/bin/sh
# Pre-commit hook template for rlvgl-creator.
#
# Scans for new or modified assets, converts them into the normalized raw
# format, and runs policy checks before allowing a commit. Integrate by copying
# or symlinking this script into your repository's `.git/hooks/pre-commit`.

set -euo pipefail

# Discover new or changed assets and update the manifest.
rlvgl-creator scan assets
# Normalize assets into raw sequences and pack fonts.
rlvgl-creator convert
# Enforce naming and path policies.
rlvgl-creator check
