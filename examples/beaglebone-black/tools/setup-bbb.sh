#!/usr/bin/env bash
# setup-bbb.sh — First-time BeagleBone Black setup over USB Ethernet.
#
# Automates:
#   1. Change the factory one-time password to a permanent one
#   2. Install SSH key for passwordless access
#   3. Probe the board (OS, kernel, devices, display state)
#
# Usage:
#   BB_FACTORY_PASSWORD=temppwd BB_PASSWORD='P0rkBone!' ./tools/setup-bbb.sh
#
# Environment variables:
#   BB_HOST             — BBB IP address (default: 192.168.6.2)
#   BB_USER             — SSH user (default: debian)
#   BB_FACTORY_PASSWORD — One-time password shown on first boot (required)
#   BB_PASSWORD         — New permanent password to set (required)
#   BB_SSH_KEY          — Path to SSH public key to install (default: ~/.ssh/id_ed25519.pub)
#
# Prerequisites:
#   - BBB connected via USB (USB Ethernet gadget at 192.168.6.2)
#   - `expect` installed (brew install expect / apt install expect)

set -euo pipefail

: "${BB_HOST:=192.168.6.2}"
: "${BB_USER:=debian}"
: "${BB_FACTORY_PASSWORD:?Set BB_FACTORY_PASSWORD to the one-time password shown at boot}"
: "${BB_PASSWORD:?Set BB_PASSWORD to the desired permanent password}"
: "${BB_SSH_KEY:=${HOME}/.ssh/id_ed25519.pub}"

SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

echo "=== BeagleBone Black Setup ==="
echo "Host: ${BB_USER}@${BB_HOST}"
echo ""

# ---------------------------------------------------------------------------
# Step 1: Change the factory password
# ---------------------------------------------------------------------------

echo "[1/4] Changing factory password..."

expect -c "
set timeout 15
spawn ssh ${SSH_OPTS} ${BB_USER}@${BB_HOST}
expect \"assword:\"
send \"${BB_FACTORY_PASSWORD}\r\"
expect {
    \"Current\" {
        send \"${BB_FACTORY_PASSWORD}\r\"
        expect \"New\"
        send \"${BB_PASSWORD}\r\"
        expect -re \"Retype|new|again\"
        send \"${BB_PASSWORD}\r\"
        expect -re \"updated|changed|closed\"
        send_user \"\n\"
    }
    \"WARNING\" {
        expect \"Current\"
        send \"${BB_FACTORY_PASSWORD}\r\"
        expect \"New\"
        send \"${BB_PASSWORD}\r\"
        expect -re \"Retype|new|again\"
        send \"${BB_PASSWORD}\r\"
        expect -re \"updated|changed|closed\"
        send_user \"\n\"
    }
    \"\\\\\$\" {
        send_user \"(password already set)\n\"
        send \"exit\r\"
    }
}
expect eof
" 2>&1 | grep -v "^spawn" || true

echo "   Password changed."

# ---------------------------------------------------------------------------
# Step 2: Verify SSH with new password
# ---------------------------------------------------------------------------

echo "[2/4] Verifying SSH access..."

VERIFY=$(expect -c "
set timeout 10
spawn ssh ${SSH_OPTS} ${BB_USER}@${BB_HOST}
expect \"assword:\"
send \"${BB_PASSWORD}\r\"
expect \"\\\\\$\"
send \"echo SSH_OK\r\"
expect \"SSH_OK\"
send \"exit\r\"
expect eof
" 2>&1 | grep "SSH_OK" || true)

if [ -z "$VERIFY" ]; then
    echo "   ERROR: Cannot SSH with new password. Check credentials."
    exit 1
fi
echo "   SSH access confirmed."

# ---------------------------------------------------------------------------
# Step 3: Install SSH key (optional)
# ---------------------------------------------------------------------------

if [ -f "${BB_SSH_KEY}" ]; then
    echo "[3/4] Installing SSH public key..."
    PUBKEY=$(cat "${BB_SSH_KEY}")

    expect -c "
    set timeout 10
    spawn ssh ${SSH_OPTS} ${BB_USER}@${BB_HOST}
    expect \"assword:\"
    send \"${BB_PASSWORD}\r\"
    expect \"\\\\\$\"
    send \"mkdir -p ~/.ssh && chmod 700 ~/.ssh && echo '${PUBKEY}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && echo KEY_INSTALLED\r\"
    expect \"KEY_INSTALLED\"
    send \"exit\r\"
    expect eof
    " 2>&1 | grep -q "KEY_INSTALLED" && echo "   SSH key installed." || echo "   WARNING: Key install may have failed."
else
    echo "[3/4] Skipping SSH key (${BB_SSH_KEY} not found)."
fi

# ---------------------------------------------------------------------------
# Step 4: Probe the board
# ---------------------------------------------------------------------------

echo "[4/4] Probing board..."
echo ""

expect -c "
set timeout 10
spawn ssh ${SSH_OPTS} ${BB_USER}@${BB_HOST}
expect \"assword:\"
send \"${BB_PASSWORD}\r\"
expect \"\\\\\$\"
send \"echo '--- OS ---'; cat /etc/os-release | head -2; echo '--- Kernel ---'; uname -r; echo '--- CPU ---'; grep 'model name' /proc/cpuinfo | head -1; echo '--- Memory ---'; free -h | head -2; echo '--- Disk ---'; df -h / | tail -1; echo '--- I2C Buses ---'; ls /dev/i2c-* 2>/dev/null; echo '--- Framebuffer ---'; ls /dev/fb* 2>/dev/null || echo '(none)'; echo '--- Input ---'; ls /dev/input/event* 2>/dev/null || echo '(none)'; echo '--- LCDC Module ---'; find /lib/modules/\$(uname -r) -name '*tilcdc*' 2>/dev/null || echo '(not found)'; echo '--- Boot Config ---'; grep -v '^#' /boot/uEnv.txt 2>/dev/null | grep -v '^\$' | head -10; echo '--- PROBE DONE ---'\r\"
expect \"PROBE DONE\"
send \"exit\r\"
expect eof
" 2>&1 | sed -n '/--- OS ---/,/--- PROBE DONE ---/p' | grep -v "PROBE DONE"

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Connect with:"
echo "  ssh ${BB_USER}@${BB_HOST}"
if [ -f "${BB_SSH_KEY}" ]; then
    echo "  (key-based auth should work)"
else
    echo "  (password: <BB_PASSWORD>)"
fi
