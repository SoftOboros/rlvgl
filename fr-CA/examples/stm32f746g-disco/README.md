<!--
examples/stm32f746g-disco/README.md - Démo de la carte STM32F746G-DISCO.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Démo STM32F746G-DISCO

Démontre la génération BSP sensible au bus sur la STM32F746G-DISCO.

## Génération BSP
Le répertoire `bsp` est rendu avec `rlvgl-creator` et sélectionne les activations AHB1/APB pour la famille F7.

## Exigences
- Cible Rust `thumbv7em-none-eabihf`
- Chaîne d'outils croisée `arm-none-eabi`

## Compilation
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32f746g-disco \
    --features "stm32f746g_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flashage
```bash
cargo objcopy --bin rlvgl-stm32f746g-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Tests manuels
1. Réinitialisez la carte et confirmez que l'interface utilisateur de la démo s'affiche correctement.
2. Exercez la saisie tactile pour vérifier que les événements atteignent les widgets.
