<!--
  MAKE.md — Developer guide for Makefile convenience targets
  Covers available targets, typical flows, and prerequisites.
-->

# Makefile Usage (Developer)

The repository includes a lightweight Makefile with convenience targets to speed up common STM32H747I-DISCO workflows: regenerating BSPs, building both cores, and managing OpenOCD.

## Prerequisites

- Rust target: `thumbv7em-none-eabihf`
  - `rustup target add thumbv7em-none-eabihf`
- Arm toolchain for debug/flash (e.g., GNU Tools for Arm Embedded)
- OpenOCD installed and in PATH

## Targets

- `make help`
  - Prints a summary of available targets.

- `make gen-stm32h747i-disco-bsp`
  - Regenerates the example BSP from `DiscoBiscuit.ioc`.
  - Defaults to `STM32_PWR_SUPPLY=SMPS` and `STM32_PWR_SDLEVEL=VOS1`.
  - Uses `examples/stm32h747i-disco/gen-bsp.sh` (idempotent; regenerates only if needed).

- `make build-disco`
  - Builds the CM7 example: `rlvgl-stm32h747i-disco`.

- `make build-disco-cm4`
  - Builds the CM4 example: `rlvgl-stm32h747i-disco-cm4`.

- `make build-disco-all`
  - Builds both CM7 and CM4 examples.

- `make openocd`
  - Starts OpenOCD with standard ST-Link + STM32H7 target scripts and halts the CPU.
  - Use this with the VSCode "CM7 attach (external OpenOCD)" configuration.

- `make openocd-erase`
  - Mass-erase via OpenOCD and exit. Use with care.

## Typical Flows

1) Regenerate BSP and build both cores

```
make gen-stm32h747i-disco-bsp
make build-disco-all
```

2) Debug using external OpenOCD (recommended)

```
make openocd                       # terminal 1
# VSCode: launch "CM7 attach (external OpenOCD)"   # terminal 2/VSCode
```

3) Update BSP power defaults (override environment)

```
STM32_PWR_SUPPLY=LDO STM32_PWR_SDLEVEL=VOS2 make gen-stm32h747i-disco-bsp
```

## Notes

- The top-level `build.rs` auto-stages the appropriate linker script for each example binary (CM7 uses `memory.x`, CM4 uses `memory_cm4.x`).
- The VSCode workspace provides two launch profiles; see `examples/stm32h747i-disco/BOOT.md` for dual-core bring-up options (A/B/C) and the mailbox-based handshake.
