<!--
examples/stm32u599i-eval/README.md - Demostración de la placa STM32U599I-EVAL.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Demostración de STM32U599I-EVAL

Muestra la generación de BSP sensible al bus en la STM32U599I-EVAL.

## Generación de BSP
El directorio `bsp` se renderiza con `rlvgl-creator` y selecciona buses RCC específicos de U5 mientras integra la limpieza de BDMA/MDMA.

## Requisitos
- Objetivo de Rust `thumbv8m.main-none-eabihf`
- Cadena de herramientas cruzadas `arm-none-eabi`

## Compilación
```bash
rustup target add thumbv8m.main-none-eabihf
cargo build --bin rlvgl-stm32u599i-eval \
    --features "stm32u599i_eval,qrcode,png,jpeg,fontdue" \
    --target thumbv8m.main-none-eabihf
```

## Flasheo
```bash
cargo objcopy --bin rlvgl-stm32u599i-eval \
    --target thumbv8m.main-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Pruebas Manuales
1. Reinicie la placa y confirme que la interfaz de usuario de la demostración se dibuja correctamente.
2. Realice entradas táctiles para verificar que los eventos llegan a los widgets.
