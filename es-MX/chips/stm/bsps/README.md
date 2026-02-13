<!--
chips/stm/bsps/README.md - Notas de generación de stub BSP de STM32.
-->
<p align="center">
  <img src="../../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-bsps-stm 🆕
Paquete: `rlvgl-bsps-stm` 🆕

Stubs de paquetes de soporte de placa para placas STM32 utilizadas por `rlvgl-creator` 🆕.
La ruta de superposición `board` heredada se mantiene por compatibilidad pero está obsoleta.
Este crate ahora incluye módulos simples generados a partir de archivos `.ioc` de CubeMX
con mapeos básicos de pines.

Regenera los stubs con `scripts/gen_ioc_bsps.sh`. El script invoca
`rlvgl-creator` 🆕 para cada `.ioc` bajo
`chips/stm/STM32_open_pin_data/boards` y escribe los módulos en
`chips/stm/bsps/src`. Los datos de MCU provienen del archivo `rlvgl-chips-stm`
incluido, por lo que no se necesita un `mcu.json` separado.

## Dispositivos compatibles

- `stm32-c0` – `dep:stm32c0xx-hal`
- `stm32-f0` – `dep:stm32f0xx-hal`
- `stm32-f3` – `dep:stm32f3xx-hal`
- `stm32-f4` – `dep:stm32f4xx-hal`
- `stm32-f7` – `dep:stm32f7xx-hal`
- `stm32-g0` – `dep:stm32g0xx-hal`
- `stm32-g4` – `dep:stm32g4xx-hal`
- `stm32-h5` – `dep:stm32h5xx-hal`
- `stm32-h7` – `dep:stm32h7xx-hal`
- `stm32-l0` – `dep:stm32l0xx-hal`
- `stm32-l1` – `dep:stm32l1xx-hal`
- `stm32-l4` – `dep:stm32l4xx-hal`
- `stm32-l5` – `dep:stm32l5xx-hal`
- `stm32-wb` – `dep:stm32wb-hal`
- `stm32-wl` – `dep:stm32wlxx-hal`

## Dispositivos no compatibles (parcial)

Se sabe que las siguientes placas no son compatibles o requieren
crates de proveedores que aún no están integrados. Son omitidas por el
script de generación de BSP.

- `stm32-n6`
- `stm32-u0`
- `stm32-u5`
- `stm32wba65i_dk1`

*Esta lista de dispositivos no compatibles no está completa; otras placas
en el archivo también pueden fallar en la compilación.*
