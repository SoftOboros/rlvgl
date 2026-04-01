# DiscoBiscuit

Firmware project targeting the **STM32H747I-DISCO** development board.

## Build

1. Ensure the GNU Arm Embedded toolchain is installed (see `codex/startup.sh`).
   `./scripts/build.sh` will automatically add the toolchain at `/opt/arm-toolchain-14.2.rel1/bin`
   to your `PATH` if it's not already present.
2. Build the CM7 firmware:
   ```bash
   ./scripts/build.sh
   ```

The resulting ELF image is located at `build/DiscoBiscuit_CM7.elf`.

## Notes

The project currently targets the CM7 core only. Additional core support
(CM4) can be added with further CMake configuration.
