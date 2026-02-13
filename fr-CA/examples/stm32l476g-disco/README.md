```markdown
<!--
examples/stm32l476g-disco/README.md - Démonstration de la carte STM32L476G-DISCO.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Démonstration STM32L476G-DISCO

Démontre la génération BSP consciente du bus pour la carte STM32L476G Discovery.

## Génération du BSP
Le répertoire `bsp` est produit par `rlvgl-creator`, sélectionnant automatiquement les registres AHB2/APB pour la famille L4.

## Exigences
- Cible Rust `thumbv7em-none-eabihf`
- Chaîne d'outils croisée `arm-none-eabi`

## Compilation
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32l476g-disco \
    --features "stm32l476g_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flashage
```bash
cargo objcopy --bin rlvgl-stm32l476g-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Tests manuels
1. Réinitialisez la carte et assurez-vous que l'interface utilisateur s'affiche.
2. Utilisez l'entrée tactile pour confirmer la gestion des événements.
```
```
