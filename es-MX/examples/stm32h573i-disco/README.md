```markdown
<!--
examples/stm32h573i-disco/README.md - Demostración de la placa STM32H573I-DISCO.
-->
<p align="centre">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Demostración STM32H573I-DISCO

Muestra la generación BSP con reconocimiento de bus en la STM32H573I-DISCO.

## Generación BSP
El directorio `bsp` se renderiza con `rlvgl-creator` y selecciona buses RCC específicos de H5 mientras integra la limpieza de BDMA/MDMA.

## Requisitos
- Objetivo de Rust `thumbv8m.main-none-eabihf`
- Cadena de herramientas cruzadas `arm-none-eabi`

## Compilación
```bash
rustup target add thumbv8m.main-none-eabihf
cargo build --bin rlvgl-stm32h573i-disco \
    --features "stm32h573i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv8m.main-none-eabihf
```

## Flasheo
```bash
cargo objcopy --bin rlvgl-stm32h573i-disco \
    --target thumbv8m.main-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Pruebas Manuales
1. Reinicie la placa y confirme que la interfaz de usuario de la demostración se dibuja correctamente.
2. Utilice la entrada táctil para verificar que los eventos llegan a los widgets.
```
```
