```markdown
<!--
examples/stm32h747i-disco/README.md - Demostración de la placa STM32H747I-DISCO.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Demostración STM32H747I-DISCO
---
Demuestra rlvgl en la placa de descubrimiento STM32H747I-DISCO usando controladores
de pantalla y táctiles de marcador de posición.

## Enlaces Rápidos
- Opciones de arranque y flujo de doble núcleo: ver `BOOT.md`
- Mapa de memoria y regiones: ver `MEMORY.md`
- Comportamiento y banderas de generación del BSP de STM32: ver `docs/STM_BSP_GENERATION.md`

## Generación del BSP
El directorio `bsp` es producido por `rlvgl-creator` y demuestra
el cierre de reloj consciente del bus. Las habilitaciones de GPIO y periféricos apuntan a
`AHB4ENR` del H7 y a los registros APB relacionados automáticamente.

```rust
use crate::bsp::{hal, pac};

let dp = pac::Peripherals::take().unwrap();
hal::init_board_hal(&dp);
```

## Requisitos
- Destino de Rust `thumbv7em-none-eabihf`
- Cadena de herramientas cruzada `arm-none-eabi`

## Construcción
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

Alternativamente, use los atajos del Makefile de nivel superior:

```
make gen-stm32h747i-disco-bsp   # Regenerar BSP (predetermina SMPS/VOS1)
make build-disco                # Construir ejemplo CM7
make build-disco-cm4            # Construir ejemplo CM4
make build-disco-all            # Construir ambos
make openocd                    # Iniciar OpenOCD (stlink + stm32h7x)
make openocd-erase              # Borrado masivo (PELIGRO)
```

Notas:
- El `build.rs` del espacio de trabajo coloca el `memory.x` de este ejemplo en el
  directorio de construcción de Cargo y pasa `-Tmemory.x` al enlazador automáticamente en
  los destinos embebidos. No se requiere un `.cargo/config.toml` global.
- El `backlight_pwm` opcional habilita el PWM del TIM8 en `PJ6` para la retroiluminación LCD. La
  construcción predeterminada usa un simple respaldo de GPIO alto/bajo para el arranque.

## Flasheo
```bash
cargo objcopy --bin rlvgl-stm32h747i-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Pruebas Manuales
1. Reinicie la placa y confirme que la interfaz de usuario de demostración coincide con el diseño del simulador.
2. Toque los widgets para asegurarse de que los eventos táctiles se propagan correctamente.

## Estado de la Pantalla (Arranque)

- Reloj de píxeles: 32 MHz (PLL3R) — valor predeterminado conservador; ajustar más tarde.
- Tiempos LTDC (típicos OTM8009A 800×480):
  - HSW=20, HBP=140, HFP=20
  - VSW=4,  VBP=34,  VFP=10
- Capa 1: framebuffer RGB565; DMA2D planeado para blits/rellenos.
- Notas:
  - Estos valores están etiquetados en `platform/src/stm32h747i_disco.rs::configure_ltdc_timing()`
    para facilitar su ajuste durante la sintonización.
  - La inicialización del panel DSI está simulada; los dibujos LTDC están en progreso.

## Táctil (FT5336)

- Bus I²C: I2C4
  - PD12 = I2C4_SCL (AF4, drenaje abierto, pull-up)
  - PD13 = I2C4_SDA (AF4, drenaje abierto, pull-up)
- Interrupción: PK7 = TOUCH_INT
- Propiedad: CM4 inicializa I2C4 y sondea FT5336; CM7 ejecuta el trabajo de visualización.
- Se agregará una inicialización de I2C4 basada en PAC para CM4; el soporte de FT5336 utiliza un
  adaptador embedded-hal 1.0.

## Retroiluminación y Reinicio (Temporal)

- Respaldo de GPIO de retroiluminación: PJ6 (alto = encendido). El arranque por PWM es opcional en TIM8
  (PJ6 soporta TIM8 CH1/CH2; enrutado a LCD_BL_CTRL).
- Reinicio del panel: PG3 (LCD_RESET en MB1166). El arranque temprano puede alternar esto a través de
  GPIO; agregue retrasos conformes a la hoja de datos.

## Opcional: Activos SD

- Habilite el adaptador FATFS no_std y el dispositivo de bloque SD al construir. Para una
  demostración mínima de listado al arrancar, habilite también `sd_assets_demo`:

```bash
cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco,fatfs_nostd,sd_assets_demo" \
    --target thumbv7em-none-eabihf --release
```

- El controlador `DiscoSdBlockDevice` (SDMMC1 + DMA + higiene de caché D) está disponible
  detrás de las características anteriores. Se incluye un adaptador `fatfs` ligero en el
  crate de la plataforma (`sd_fatfs_adapter`). Con `sd_assets_demo`, el firmware
  intentará montar y listar `/assets` al inicio y renderizar algunos nombres.

### Indicadores en pantalla

- `asset: <nombre>`: FAT montado y `/assets` contiene entradas; se muestran hasta 4.
- `SD: no assets`: FAT montado pero `/assets` (o raíz) está vacío.
- `SD: mount/list failed`: Falló el montaje de FAT o el listado del directorio (verificar pines/reloj/tarjeta SD).
```
