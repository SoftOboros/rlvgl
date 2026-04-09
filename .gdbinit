# .gdbinit — Workspace GDB helper macros for rlvgl bring-up
#
# Loaded automatically by VS Code Cortex-Debug (preLaunchCommands) and
# by scripts/start-gdb.sh. Contains quick macros for fault inspection,
# escaping helper functions via the link register, and SDRAM checks.

# Dump Cortex-M fault registers
define faultregs
  printf "CFSR = 0x%08x\n", *((unsigned int*)0xE000ED28)
  printf "HFSR = 0x%08x\n", *((unsigned int*)0xE000ED2C)
  printf "MMFAR= 0x%08x\n", *((unsigned int*)0xE000ED34)
  printf "BFAR = 0x%08x\n", *((unsigned int*)0xE000ED38)
end

# Break at caller return address (escape helpers like send_command)
define lrtrap
  tbreak *$lr
  continue
end

# Safer variant: attempts to recover a plausible return address from the stack
# if $lr looks bogus (e.g., 0x3b9aca00) due to prologue having spilled LR.
define lrtrap_safe
  set $addr = $lr
  if ($addr < 0x08000001 || $addr > 0x0FFFFFFF)
    set $i = 0
    set $addr = 0
    while ($i < 64)
      set $cand = *(unsigned int*)($sp + $i)
      if ($cand >= 0x08000001 && $cand <= 0x0FFFFFFF)
        set $addr = $cand
        break
      end
      set $i = $i + 4
    end
  end
  if ($addr != 0)
    tbreak *$addr
    continue
  else
    printf "[lrtrap_safe] could not recover LR (lr=0x%08x)\n", $lr
  end
end

# Peek stack
define peekstack
  x/16wx $sp
end

# Dump registers
define regs
  info registers
end

# Convenient aliases (not actual key bindings in VS Code)
define fr
  faultregs
end
define lrt
  lrtrap
end
define ps
  peekstack
end

# Optional: SDRAM FMC register snapshot
define sdramregs
  printf "SDCMR = 0x%08x\n", *((unsigned int*)0x52004140)
  printf "SDSR  = 0x%08x\n", *((unsigned int*)0x52004158)
  printf "SDCR1 = 0x%08x\n", *((unsigned int*)0x52004080)
  printf "SDCR2 = 0x%08x\n", *((unsigned int*)0x52004084)
  printf "SDTR1 = 0x%08x\n", *((unsigned int*)0x52004104)
  printf "SDTR2 = 0x%08x\n", *((unsigned int*)0x52004108)
end

# Optional: wait for SDSR.BUSY to clear with bounded polls
define wait_busy_clear
  set $i = 0
  while ($i < 1000)
    set $sdsr = *((unsigned int*)0x52004158)
    if (($sdsr & 0x20) == 0)
      printf "SDSR.BUSY cleared after %d polls (SDSR=0x%08x)\n", $i, $sdsr
      break
    end
    set $i = $i + 1
    # single-step for a tiny delay without timers
    stepi
  end
  if ($i >= 1000)
    printf "SDSR.BUSY still set (SDSR=0x%08x)\n", $sdsr
  end
end

# Optional: hardware watchpoint to trap writes to FMC SDCMR
define watch_sdcmd
  hwatch *(unsigned int*)0x52004140
end
