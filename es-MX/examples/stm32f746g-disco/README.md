<!--
examples/stm32f746g-disco/README.md - Demostración de la placa STM32F746G-DISCO.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Demostración STM32F746G-DISCO

Demuestra la generación de BSP consciente del bus en la STM32F746G-DISCO.

## Generación de BSP
El directorio `bsp` se renderiza con `rlvgl-creator` y selecciona las activaciones AHB1/APB para la familia F7.

## Requisitos
- Destino de Rust `thumbv7em-none-eabihf`
- Cadena de herramientas cruzada `arm-none-eabi`

## Compilación
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32f746g-disco \
    --features "stm32f746g_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flasheo
```bash
cargo objcopy --bin rlvgl-stm32f746g-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Pruebas manuales
1. Reinicie la placa y confirme que la interfaz de usuario de demostración se dibuja correctamente.
2. Pruebe la entrada táctil para verificar que los eventos lleguen a los widgets.
