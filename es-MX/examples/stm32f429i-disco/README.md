```markdown
<!--
examples/stm32f429i-disco/README.md - Demostración de la placa STM32F429I-DISCO.
-->
<p align="centre">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Demostración de STM32F429I-DISCO

Muestra rlvgl en la placa STM32F429I-DISCO usando la generación de BSP consciente del bus.

## Generación de BSP
El directorio `bsp` se renderiza con `rlvgl-creator` y selecciona automáticamente
los registros AHB1/APB apropiados para la familia F4.

## Requisitos
- Destino de Rust `thumbv7em-none-eabihf`
- Cadena de herramientas cruzadas `arm-none-eabi`

## Construcción
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32f429i-disco \
    --features "stm32f429i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flasheo
```bash
cargo objcopy --bin rlvgl-stm32f429i-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Pruebas manuales
1. Reinicie la placa y confirme que la interfaz de usuario de la demostración se dibuja correctamente.
2. Utilice la entrada táctil para verificar que los eventos llegan a los widgets.
```
