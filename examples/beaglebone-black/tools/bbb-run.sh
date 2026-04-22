#!/usr/bin/env bash
# bbb-run.sh — Drive the BBB (reserve fb, unbind tilcdc, run rlvgl-bbb,
# dump LCDC registers) over SSH with sudo-via-stdin auth.
#
# The BBB is a remote target; the sudo password lives only in your
# shell's BBB_SUDO_PASS env var:
#
#   read -s BBB_SUDO_PASS && export BBB_SUDO_PASS
#
# Password handling:
#   - Transmitted over the encrypted SSH channel only.
#   - Never appears in argv on the Mac or the BBB (we pass it via
#     bash `<<<` here-string into ssh stdin → `sudo -S` stdin).
#   - Never written to any file.
#
# Usage:
#   bash bbb-run.sh <subcommand>
#
# Subcommands:
#   check     Report cmdline, iomem, /dev/fb0 state (no sudo)
#   reserve   Run reserve-fb.sh (sudo)
#   reboot    Reboot and wait for SSH to return (sudo)
#   unbind    Unbind tilcdc (sudo)
#   run       Run rlvgl-bbb in the foreground (sudo, long-running)
#   regs      Dump LCDC registers via /dev/mem (sudo)
#   smoke     unbind + run
#   ship      reserve + reboot + unbind + run (first-time full sequence)
#
# Env:
#   BBB_HOST        ssh target (default: debian@192.168.6.2)
#   BBB_SUDO_PASS   sudo password (required for sudo-ful subcommands)
#   BBB_BIN         remote binary path (default: ~/rlvgl-bbb)

set -euo pipefail

BBB_HOST="${BBB_HOST:-debian@192.168.6.2}"
BBB_BIN="${BBB_BIN:-rlvgl-bbb}"

die() { echo "error: $*" >&2; exit 1; }

need_pass() {
    [ -n "${BBB_SUDO_PASS:-}" ] \
        || die "BBB_SUDO_PASS not set; run:  read -s BBB_SUDO_PASS && export BBB_SUDO_PASS"
}

# Run a plain ssh command on the BBB (no sudo).
bbb_ssh() {
    ssh "$BBB_HOST" "$@"
}

# Run a command on the BBB under sudo -S. The password is supplied via
# ssh stdin (bash `<<<` here-string) and consumed by the first `sudo -S`
# call; later sudo calls within the same command reuse cached credentials.
#
# `$REMOTE_HOME` is exported to the sudo'd shell (sudo's default reset
# of HOME would otherwise point at /root). Reference it as
# `$REMOTE_HOME` inside the command.
bbb_sudo() {
    local remote_cmd="$1"
    need_pass
    # Resolve the SSH user's home dir, then run the command under sudo
    # with REMOTE_HOME pre-exported so it survives sudo's env reset.
    ssh "$BBB_HOST" \
        "REMOTE_HOME=\$HOME && sudo -S -p '' env REMOTE_HOME=\$REMOTE_HOME sh -c $(printf '%q' "$remote_cmd")" \
        <<< "$BBB_SUDO_PASS"
}

# ---------------------------------------------------------------------------
# Subcommands
# ---------------------------------------------------------------------------

cmd_check() {
    bbb_ssh bash -s <<'EOSSH'
set -euo pipefail
echo "=== /proc/cmdline ==="
tr ' ' '\n' < /proc/cmdline | grep -E '^(mem=|console=|root=)' || true
echo ""
echo "=== /proc/iomem (top of RAM) ==="
grep -iE 'system ram|reserved' /proc/iomem | tail -6 || true
echo ""
echo "=== /dev/fb0 ==="
if [ -e /dev/fb0 ]; then
    ls -la /dev/fb0
    echo "(tilcdc still bound to LCDC)"
else
    echo "/dev/fb0 absent (tilcdc unbound — LCDC is yours)"
fi
echo ""
echo "=== rlvgl-bbb binary ==="
ls -la "$HOME/rlvgl-bbb" 2>/dev/null || echo "rlvgl-bbb not deployed"
EOSSH
}

cmd_reserve() {
    bbb_sudo 'bash "$REMOTE_HOME/reserve-fb.sh"'
}

cmd_reboot() {
    # `reboot` severs the SSH connection, which normally makes ssh exit
    # nonzero. Swallow that with `|| true` and then poll for reconnection.
    bbb_sudo 'reboot' || true
    echo "Waiting for BBB to come back..."
    sleep 10
    for i in $(seq 1 30); do
        if ssh -o ConnectTimeout=2 -o BatchMode=yes "$BBB_HOST" true 2>/dev/null; then
            echo "BBB back online"
            return 0
        fi
        sleep 2
    done
    die "BBB did not come back within 70s"
}

cmd_unbind() {
    bbb_sudo 'bash "$REMOTE_HOME/unbind-tilcdc.sh"'
}

cmd_run() {
    echo "Running rlvgl-bbb on $BBB_HOST (Ctrl-C to stop)..."
    bbb_sudo "exec \"\$REMOTE_HOME/$BBB_BIN\""
}

cmd_regs() {
    bbb_sudo "python3 -c \"
import mmap, os, struct
fd = os.open('/dev/mem', os.O_RDONLY)
m = mmap.mmap(fd, 0x1000, mmap.MAP_SHARED, mmap.PROT_READ, offset=0x4830E000)
labels = {
    0x04: 'LCD_CTRL          ',
    0x28: 'RASTER_CTRL       ',
    0x2C: 'RASTER_TIMING_0   ',
    0x30: 'RASTER_TIMING_1   ',
    0x34: 'RASTER_TIMING_2   ',
    0x40: 'LCDDMA_CTRL       ',
    0x44: 'LCDDMA_FB0_BASE   ',
    0x48: 'LCDDMA_FB0_CEILING',
    0x58: 'IRQSTATUS_RAW     ',
    0x5C: 'IRQSTATUS         ',
    0x60: 'IRQENABLE_SET     ',
    0x6C: 'CLKC_ENABLE       ',
}
for off in sorted(labels):
    v = struct.unpack('<I', m[off:off+4])[0]
    print(f'0x4830E0{off:02X} {labels[off]} = 0x{v:08X}')
\""
}

cmd_smoke() {
    cmd_check
    echo ""
    echo "=== unbind tilcdc ==="
    cmd_unbind
    echo ""
    echo "=== launching rlvgl-bbb ==="
    cmd_run
}

cmd_ship() {
    echo "=== reserve framebuffer ==="
    cmd_reserve
    echo ""
    echo "=== reboot ==="
    cmd_reboot
    echo ""
    echo "=== verify reservation ==="
    cmd_check
    echo ""
    echo "=== unbind tilcdc ==="
    cmd_unbind
    echo ""
    echo "=== launching rlvgl-bbb ==="
    cmd_run
}

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------

cmd="${1:-}"
case "$cmd" in
    check|reserve|reboot|unbind|run|regs|smoke|ship)
        cmd_"$cmd"
        ;;
    "")
        sed -n '2,30p' "$0"
        exit 1
        ;;
    *)
        die "unknown subcommand: $cmd"
        ;;
esac
