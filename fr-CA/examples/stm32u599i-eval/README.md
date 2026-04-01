```markdown
<!--
examples/stm32u599i-eval/README.md - Démonstration de la carte STM32U599I-EVAL.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Démonstration STM32U599I-EVAL

Présente la génération de BSP sensible au bus sur la carte STM32U599I-EVAL.

## Génération BSP
Le répertoire `bsp` est rendu avec `rlvgl-creator` et sélectionne les bus RCC spécifiques à l'U5 tout en intégrant le nettoyage BDMA/MDMA.

## Exigences
- Cible Rust `thumbv8m.main-none-eabihf`
- Chaîne d'outils croisée `arm-none-eabi`

## Compilation
```bash
rustup target add thumbv8m.main-none-eabihf
cargo build --bin rlvgl-stm32u599i-eval \
    --features "stm32u599i_eval,qrcode,png,jpeg,fontdue" \
    --target thumbv8m.main-none-eabihf
```

## Flashage
```bash
cargo objcopy --bin rlvgl-stm32u599i-eval \
    --target thumbv8m.main-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Test manuel
1. Réinitialisez la carte et confirmez que l'interface utilisateur de la démo s'affiche correctement.
2. Exercez l'entrée tactile pour vérifier que les événements atteignent les widgets.
```
