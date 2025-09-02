<!--
examples/stm32h747i-disco/DEBUG-SETUP.md - STM32 debug setup (OpenOCD, GDB), macOS LaunchAgent, and recovery recipes.
-->

# Debug Setup

This guide documents a reliable OpenOCD + GDB workflow for STM32H7 boards (e.g., STM32H747I-DISCO), including a macOS LaunchAgent to keep OpenOCD running, a resilient GDB auto-reconnect flow, and recovery commands when SWD/JTAG gets into a bad state.

## macOS LaunchAgent for OpenOCD

Create a LaunchAgent to start OpenOCD at login and restart it if it crashes.

1) Create `~/Library/LaunchAgents/com.softoboros.openocd.plist` with the content below.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.softoboros.openocd</string>

  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/openocd</string>
    <string>-f</string><string>interface/stlink.cfg</string>
    <string>-f</string><string>target/stm32h7x.cfg</string>
    <string>-c</string>
    <string>transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init</string>
  </array>

  <!-- restart if it dies -->
  <key>KeepAlive</key>
  <true/>

  <!-- log files -->
  <key>StandardOutPath</key>
  <string>/tmp/openocd.out.log</string>
  <key>StandardErrorPath</key>
  <string>/tmp/openocd.err.log</string>

  <!-- start at login -->
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
```

- Adjust `/usr/local/bin/openocd` to where Homebrew installed it: `$(brew --prefix open-ocd)/bin/openocd`.

2) Load and unload:

```bash
launchctl load ~/Library/LaunchAgents/com.softoboros.openocd.plist
launchctl unload ~/Library/LaunchAgents/com.softoboros.openocd.plist
```

3) Status and logs:

```bash
launchctl list | grep openocd
tail -f /tmp/openocd.out.log /tmp/openocd.err.log
```

## GDB Server Setup (Auto‑Reconnect)

Two resilient approaches to keep GDB attached to OpenOCD even if the SWD wire drops.

### Option A — Shell wrapper that relaunches GDB

`debug-gdb.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
while :; do
  gdb -q --command=gdb_init.gdb || true
  echo "[gdb] disconnected; retrying in 1s"; sleep 1
done
```

`gdb_init.gdb` (lean, resilient):

```gdb
set pagination off
set confirm off
set target-async on
set remotetimeout 2

# connect; if OpenOCD is not ready yet, GDB exits and wrapper restarts it
target extended-remote :3333

# typical STM32H7 session warmup
monitor reset halt
monitor arm semihosting enable

# stay attached; if the wire drops, GDB exits → wrapper relaunches
```

### Option B — In‑GDB “retry connect” loop (no wrapper)

```gdb
set pagination off
set confirm off
set target-async on
set remotetimeout 1

define _reconnect
  while 1
    echo [gdb] trying :3333...\n
    if $_target_connected
      monitor reset halt
      break
    end
    set timeout 1
    target extended-remote :3333
    sleep 1
  end
end

_reconnect
```

If your GDB lacks `$_target_connected`, prefer the shell wrapper.

Nice‑to‑haves:

- Keep ports stable: configure `gdb_port 3333`, `telnet_port 4444`, `tcl_port 6666` in your OpenOCD command or `.cfg`.
- Backoff: if the probe is unplugged, add exponential sleep to avoid log spam.
- Health check: a tiny watchdog can `nc -z localhost 3333` and restart if needed (mostly redundant with LaunchAgent/systemd).
- Target quirks (H7): slow down and connect‑under‑reset often helps:
  `-c "transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init"`

## Debug Recovery (STM32H7)

If OpenOCD fails during “examine” (e.g., can’t read `DBGMCU` at `0x5C001004`) and SWD/JTAG gets invalid, it’s usually connect timing, too‑fast SWD clock, or a protected/low‑power target. Try, in order:

1) Slow SWD + connect‑under‑reset (most common fix):

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; flash erase_address 0x08000000 0x200000; exit"
```

2) Use the driver’s mass‑erase (bank 0; add bank 1 if dual‑bank):

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x mass_erase 0; exit"
```

For dual‑bank parts, also run:

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x mass_erase 1; exit"
```

3) If the part is debug‑locked (RDP), unlock (this mass‑erases):

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x unlock 0; exit"
```

