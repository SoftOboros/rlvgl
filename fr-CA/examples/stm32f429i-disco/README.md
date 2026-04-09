<!--
examples/stm32f429i-disco/README.md - Démo de la carte STM32F429I-DISCO.
-->
<p align="centre">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Démo STM32F429I-DISCO

Présente rlvgl sur la carte STM32F429I-DISCO en utilisant la génération BSP compatible bus.

## Génération BSP
Le répertoire `bsp` est rendu avec `rlvgl-creator` et sélectionne automatiquement
les registres AHB1/APB appropriés pour la famille F4.

## Prérequis
- Cible Rust `thumbv7em-none-eabihf`
- Chaîne d'outils croisée `arm-none-eabi`

## Compilation
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32f429i-disco \
    --features "stm32f429i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flashage
```bash
cargo objcopy --bin rlvgl-stm32f429i-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Test manuel
1. Réinitialisez la carte et confirmez que l'interface utilisateur de la démo s'affiche correctement.
2. Exercez l'entrée tactile pour vérifier que les événements atteignent les widgets.
