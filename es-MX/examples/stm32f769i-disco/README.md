```markdown
<!--
examples/stm32f769i-disco/README.md - Demostración de la tarjeta STM32F769I-DISCO.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Demostración de STM32F769I-DISCO

Muestra la generación de BSP consciente del bus en la STM32F769I-DISCO.

## Generación de BSP
El directorio `bsp` se renderiza con `rlvgl-creator` y selecciona las activaciones AHB1/APB para la familia F7 mientras integra la limpieza de BDMA/MDMA.

## Requisitos
- Destino de Rust `thumbv7em-none-eabihf`
- Cadena de herramientas cruzadas `arm-none-eabi`

## Construcción
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32f769i-disco \
    --features "stm32f769i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flasheo
```bash
cargo objcopy --bin rlvgl-stm32f769i-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Pruebas manuales
1. Reinicie la placa y confirme que la interfaz de usuario de demostración se dibuja correctamente.
2. Utilice la entrada táctil para verificar que los eventos lleguen a los widgets.
```
