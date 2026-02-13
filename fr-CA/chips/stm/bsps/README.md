<!--
chips/stm/bsps/README.md - Notes de génération de stubs BSP STM32.
-->
<p align="center">
  <img src="../../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-bsps-stm 🆕
Paquet : `rlvgl-bsps-stm` 🆕

Stubs de paquet de support de carte pour les cartes STM32 utilisés par `rlvgl-creator` 🆕.
Le chemin de superposition `board` hérité est conservé pour la compatibilité, mais est déprécié.
Ce crate inclut désormais des modules simples générés à partir de fichiers CubeMX `.ioc`
avec des mappages de broches de base.

Régénérez les stubs avec `scripts/gen_ioc_bsps.sh`. Le script invoque
`rlvgl-creator` 🆕 pour chaque `.ioc` sous
`chips/stm/STM32_open_pin_data/boards` et écrit les modules dans
`chips/stm/bsps/src`. Les données MCU proviennent de l'archive `rlvgl-chips-stm`
fournie, donc aucun `mcu.json` séparé n'est nécessaire.

## Appareils pris en charge

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

## Appareils non pris en charge (partiel)

Les cartes suivantes sont connues pour ne pas être prises en charge ou nécessitent des crates
fournisseurs qui ne sont pas encore intégrés. Elles sont ignorées par le script
de génération de BSP.

- `stm32-n6`
- `stm32-u0`
- `stm32-u5`
- `stm32wba65i_dk1`

*Cette liste d'appareils non pris en charge n'est pas complète ; d'autres cartes
dans l'archive peuvent également échouer à la compilation.*
