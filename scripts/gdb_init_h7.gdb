set pagination off
set confirm off
set target-async on
set remotetimeout 2
# connect; if OpenOCD not ready yet, GDB exits and wrapper restarts it
target extended-remote :3333

# typical STM32H7 session warmup
monitor reset halt
monitor arm semihosting enable
# stay attached; if the wire drops, GDB exits → wrapper relaunches
