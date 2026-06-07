<!--
chipdb/README.md - Index of vendor chip database crates.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Vendor chip databases

Crates that embed vendor-specific chip and board data for rlvgl-creator.

## BSP generation status

| Crate | Vendor | BSP Generator | Pin Model | Chips | Boards |
|-------|--------|--------------|-----------|-------|--------|
| rlvgl-chips-esp | Espressif | Full | IO MUX + GPIO matrix | 9 | 14 |
| rlvgl-chips-nrf | Nordic | Full | PSEL registers | 1 | 1 |
| rlvgl-chips-nxp | NXP | Full | IOMUX ALT + daisy chain | 1 | 1 |
| rlvgl-chips-rp2040 | Raspberry Pi | Full | FUNCSEL per GPIO | 1 | 1 |
| rlvgl-chips-renesas | Renesas | Full | PFS PSEL + PMR | 1 | 1 |
| rlvgl-chips-stm | STMicroelectronics | CubeMX .ioc | AF mux | many | many |
| rlvgl-chips-microchip | Microchip | Stub | — | — | 1 |
| rlvgl-chips-silabs | Silicon Labs | Stub | — | — | 1 |
| rlvgl-chips-ti | Texas Instruments | Stub | — | — | 1 |

## Crates
- [rlvgl-chips-esp](./rlvgl-chips-esp/README.md) – Espressif chip database.
- [rlvgl-chips-microchip](./rlvgl-chips-microchip/README.md) – Microchip chip database.
- [rlvgl-chips-nrf](./rlvgl-chips-nrf/README.md) – Nordic chip database.
- [rlvgl-chips-nxp](./rlvgl-chips-nxp/README.md) – NXP chip database.
- [rlvgl-chips-renesas](./rlvgl-chips-renesas/README.md) – Renesas chip database.
- [rlvgl-chips-rp2040](./rlvgl-chips-rp2040/README.md) – RP2040 chip database.
- [rlvgl-chips-silabs](./rlvgl-chips-silabs/README.md) – Silicon Labs chip database.
- [rlvgl-chips-stm](./rlvgl-chips-stm/README.md) – STMicroelectronics chip database.
- [rlvgl-chips-ti](./rlvgl-chips-ti/README.md) – Texas Instruments chip database.
