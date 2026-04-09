```markdown
<!--
examples/stm32h573i-disco/README.md - Démonstration de la carte STM32H573I-DISCO.
-->
<p align="centre">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Démonstration STM32H573I-DISCO

Présente la génération BSP (Board Support Package) compatible avec le bus sur la STM32H573I-DISCO.

## Génération BSP
Le répertoire `bsp` est généré avec `rlvgl-creator` et sélectionne les bus RCC spécifiques au H5 tout en intégrant le nettoyage BDMA/MDMA.

## Prérequis
- Cible Rust `thumbv8m.main-none-eabihf`
- Chaîne d'outils croisée `arm-none-eabi`

## Compilation
```bash
rustup target add thumbv8m.main-none-eabihf
cargo build --bin rlvgl-stm32h573i-disco \
    --features "stm32h573i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv8m.main-none-eabihf
```

## Flashage
```bash
cargo objcopy --bin rlvgl-stm32h573i-disco \
    --target thumbv8m.main-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Tests manuels
1. Réinitialisez la carte et confirmez que l'interface utilisateur de la démo s'affiche correctement.
2. Exercez l'entrée tactile pour vérifier que les événements atteignent les widgets.
```
```
