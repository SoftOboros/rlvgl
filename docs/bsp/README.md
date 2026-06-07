# BSP and Chip Support

Board-support-package generation and vendor chip support design.

## Documents

- [STM32.md](./STM32.md) — STM32 BSP generation behavior, flags, and roadmap.
- [IOC-IR-ALIGNMENT.md](./IOC-IR-ALIGNMENT.md) — aligning CubeMX `.ioc` data with the internal IR.
- [CHIP-SUPPORT.md](./CHIP-SUPPORT.md) — vendor chip/board support: IR per vendor, parsers, chipdb.

## See also

- [../creator/](../creator/) — the `rlvgl-creator` binary that consumes
  chipdbs and emits BSPs.
- [../../chipdb/](../../chipdb/README.md) — vendor chipdb crates.
