#!/usr/bin/env bash
# deploy-bbb.sh — rsync the rlvgl workspace to the BBB and build rlvgl-bbb there.
#
# Building on-board avoids needing a Linux cross-toolchain on macOS. The
# BBB's Bookworm image has gcc; this script makes sure `cargo` is also
# available, then builds the linux-feature release binary.
#
# Env:
#   BBB_HOST      ssh target (default: debian@192.168.6.2)
#   BBB_DIR       remote workspace path (default: ~/rlvgl)
#   BBB_PROFILE   cargo profile (default: release)
#
# Usage:
#   bash tools/deploy-bbb.sh
#   BBB_HOST=debian@myboard.local bash tools/deploy-bbb.sh

set -euo pipefail

BBB_HOST="${BBB_HOST:-debian@192.168.6.2}"
# Remote path, relative to the SSH user's $HOME on the BBB.
BBB_DIR="${BBB_DIR:-rlvgl}"
BBB_PROFILE="${BBB_PROFILE:-release}"

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"

echo "=== rlvgl-bbb deploy ==="
echo "  repo:    $REPO_ROOT"
echo "  host:    $BBB_HOST"
echo "  remote:  $BBB_DIR"
echo "  profile: $BBB_PROFILE"
echo ""

# 1. Verify SSH reachability (bail early on host-key trouble so the user
#    can clear ~/.ssh/known_hosts before we rsync a GB of source).
ssh -o BatchMode=yes -o ConnectTimeout=5 "$BBB_HOST" true

# 2. Ensure rustup is installed on the BBB (one-time per fresh SD card).
ssh "$BBB_HOST" bash -s <<'EOSSH'
set -euo pipefail
if ! command -v cargo >/dev/null 2>&1; then
    echo "[bbb] Installing rustup (one-time)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable
fi
source "$HOME/.cargo/env"
rustc --version
EOSSH

# 3. rsync sources (skip target/ and .git/ to keep it small).
rsync -avz --delete \
    --exclude='/target' \
    --exclude='/.git' \
    --exclude='.DS_Store' \
    --exclude='**/target/' \
    "$REPO_ROOT/" "$BBB_HOST:$BBB_DIR/"

# 4. Build on the BBB.
ssh "$BBB_HOST" bash -s <<EOSSH
set -euo pipefail
source "\$HOME/.cargo/env"
cd $BBB_DIR
RUSTFLAGS="" cargo build --$BBB_PROFILE \
    -p rlvgl-example-bbb --features linux --bin rlvgl-bbb
file target/$BBB_PROFILE/rlvgl-bbb
ls -la target/$BBB_PROFILE/rlvgl-bbb
EOSSH

echo ""
echo "=== Build complete ==="
echo ""
echo "Next steps (all run on the BBB; ssh $BBB_HOST first):"
echo ""
echo "  # one-time, first run only:"
echo "  sudo bash ~/$BBB_DIR/examples/beaglebone-black/tools/reserve-fb.sh"
echo "  sudo reboot"
echo ""
echo "  # every subsequent run:"
echo "  sudo bash ~/$BBB_DIR/examples/beaglebone-black/tools/unbind-tilcdc.sh"
echo "  sudo ~/$BBB_DIR/target/$BBB_PROFILE/rlvgl-bbb"
