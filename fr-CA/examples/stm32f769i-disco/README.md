<!--
examples/stm32f769i-disco/README.md - Démo de la carte STM32F769I-DISCO.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Démo STM32F769I-DISCO

Présente la génération BSP (Board Support Package) compatible bus sur la carte STM32F769I-DISCO.

## Génération BSP
Le répertoire `bsp` est rendu avec `rlvgl-creator` et sélectionne les activations AHB1/APB pour la famille F7 tout en intégrant le nettoyage BDMA/MDMA.

## Exigences
- Cible Rust `thumbv7em-none-eabihf`
- Chaîne d'outils croisée `arm-none-eabi`

## Compilation
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32f769i-disco \
    --features "stm32f769i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flashage
```bash
cargo objcopy --bin rlvgl-stm32f769i-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Test manuel
1. Réinitialisez la carte et confirmez que l'interface utilisateur de la démo s'affiche correctement.
2. Utilisez l'entrée tactile pour vérifier que les événements atteignent les widgets.
