set pagination off
set confirm off
set target-async on
set remotetimeout 1

# Try to connect repeatedly until OpenOCD accepts
define _reconnect
  while 1
    if $_target_connected
      break
    end
    echo [gdb] trying :3333...\n
    target extended-remote :3333
    if $_target_connected
      break
    end
    sleep 1
  end
end

_reconnect

# Ensure target is halted and semihosting on
monitor reset halt
monitor arm semihosting enable

# Load the current ELF image to target RAM/flash
load

# Break at main so we can advance by source line reliably
tbreak main
continue

# Run to just before GPIO pin init
advance examples/stm32h747i-disco/src/main.rs:168

echo \n[gdb] Reached line 168 (pre-pin init).\n
list 162,172
info line
info reg pc
