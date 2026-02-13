```markdown
<!--
examples/stm32l476g-disco/README.md - Demostración de la placa STM32L476G-DISCO.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Demostración STM32L476G-DISCO

Demuestra la generación de BSP consciente del bus para la placa STM32L476G Discovery.

## Generación de BSP
El directorio `bsp` es producido por `rlvgl-creator`, seleccionando automáticamente los registros AHB2/APB para la familia L4.

## Requisitos
- Target de Rust `thumbv7em-none-eabihf`
- Toolchain cruzado `arm-none-eabi`

## Construcción
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32l476g-disco \
    --features "stm32l476g_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flasheo
```bash
cargo objcopy --bin rlvgl-stm32l476g-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Pruebas Manuales
1. Reinicie la placa y asegúrese de que la interfaz de usuario se renderice.
2. Use la entrada táctil para confirmar el manejo de eventos.
```
