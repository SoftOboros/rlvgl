```markdown
<!--
TODO-STM32H747I-DISCO.md - Lista de verificación de arranque y plan de trabajo para hardware real.
-->

# Tareas pendientes de arranque de hardware STM32H747I-DISCO

Este documento rastrea el trabajo restante necesario para ejecutar la demostración `rlvgl` en hardware real STM32H747I-DISCO (núcleo M7). Los elementos se agrupan por subsistema y se ordenan aproximadamente desde los prerrequisitos de arranque hasta las funciones de nivel superior.

## Arranque, Enlace y Relojes

- Script de construcción para el script del enlazador:
  - Estado: hecho. El `build.rs` del espacio de trabajo ahora copia
    `examples/stm32h747i-disco/memory.x` en `OUT_DIR`, emite
    `cargo:rustc-link-search` y `cargo:rustc-link-arg=-Tmemory.x` para el
    objetivo embebido. Esto sigue las guías del proyecto "Ejemplos de scripts del enlazador"
    evitando cualquier suposición global de `.cargo/config.toml`.
    Si el ejemplo se divide alguna vez en su propio crate, refleje esta lógica mínima
    en un `build.rs` local.
- Relojes del sistema y PLLs:
  - Analizar la configuración del PLL del `.ioc` (ya soportado en IR) y generar
    una configuración de reloj mínima suficiente para el reloj de píxeles LTDC y los núcleos I2C/SDMMC.
  - Programar los PLLs y los muxes del núcleo a través de PAC/HAL durante la inicialización de la placa.

## SDRAM Externa (FMC)

- Implementar la inicialización del controlador SDRAM (tiempos, registros de modo, refresco):
  - Configurar los pines FMC y la temporización para la SDRAM integrada.
  - Ejecutar la secuencia de inicialización JEDEC SDRAM y establecer la tasa de refresco.
  - Verificar que la base del framebuffer en `0xC000_0000` sea escribible y estable.

## Pantalla (LTDC + DSI + Panel)

- Configuración de temporización y capa de LTDC:
  - Programar anchos de sincronización, back/front porch y polaridad para el panel de 800×480.
  - Configurar el paso de la capa 1, formato de píxeles (RGB565), mezcla y habilitar la recarga.
- Arranque del enlace MIPI-DSI:
  - Ampliar `platform::otm8009a` para incluir la secuencia completa de inicialización del panel (formato,
    encendido, gamma) en lugar del mínimo actual de sleep-out/display-on.
  - Configurar los parámetros del modo de video del host DSI e iniciar el enlace.
- Ruta de vaciado (flush):
  - Implementar `DisplayDriver::flush` para transferir los cambios a la SDRAM y/o activar
    la recarga de LTDC. Considerar la aceleración DMA2D si está disponible (característica opcional).

## Retroiluminación y Reinicio del Panel

- PWM de retroiluminación:
  - Progreso: existe una retroiluminación de respaldo HAL-GPIO en el ejemplo y una
    función `backlight_pwm` que habilita una ruta PWM HAL TIM8 con un adaptador
    `SetDutyCycle` de embedded-hal 1.0. Se implementa un aumento suave del brillo
    al inicio en el arranque de la pantalla. Siguiente: considerar hacer que PWM sea el predeterminado.
- GPIO de reinicio del panel:
  - Progreso: `PG3` (LCD_RESET) se controla a través de HAL GPIO en el ejemplo con un
    retraso básico entre bajo/alto, antes de la inicialización de DSI. Siguiente: reemplazar
    el retraso de ciclo grueso con un retraso basado en temporizador que coincida con
    la temporización de la hoja de datos.

## Táctil (FT5336)

- Cableado real de I2C4:
  - Confirmar que `.ioc` tiene I2C4 SCL/SDA en `PD12/PD13` (AF4, drenaje abierto, pull-ups).
  - Estado: hecho. Existe una función de ayuda de inicialización HAL
    (`platform::stm32h747i_disco::init_touch_i2c`) y mapea PD12/PD13 a AF4
    drenaje abierto con una velocidad de bus de 400 kHz.
  - Eliminar el shim temporal de compatibilidad I2C 0.2→1.0 una vez que la plataforma/HAL
    converjan en embedded-hal 1.0 para I2C.
- Línea de interrupción (opcional):
  - Conectar FT5336 INT en `PK7` como entrada y usar la ruta `new_with_int` para reducir
    el sondeo.

## Tarjeta SD (opcional)

- Validar `DiscoSdBlockDevice` contra medios reales:
  - Progreso: `platform::DiscoSdBlockDevice` se implementa utilizando HAL SDMMC1
    con mantenimiento explícito de D-Cache y un tamaño de bloque de 512 bytes. Siguiente: validar
    en hardware e integrar `fatfs` detrás de la función `fs` en el ejemplo.
  - Lista de verificación:
    - Configurar GPIO: `PC8..PC12` → AF12, `PD2` → AF12; velocidad muy alta, pull-ups.
    - Reloj: Habilitar el reloj del núcleo `SDMMC1` (se recomienda PLL2 `Q`), habilitar DMA.
    - Inicialización HAL: construir `stm32h7xx_hal::sdmmc::Sdmmc` con flujos DMA RX/TX.
    - Envolver como `DiscoSdBlockDevice` y montar a través de `fatfs` (adaptador) para listar `/assets`.
  - Seguimiento: agregar una pequeña demostración en el dispositivo que monte, liste `/assets` y renderice
    una línea de texto o una imagen como prueba de humo.

### Esquema de arranque de SDMMC1 (HAL)

```rust
// GPIO & clocks (abrev.)
let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);
let _d0 = gpioc.pc8.into_alternate::<12>();
let _d1 = gpioc.pc9.into_alternate::<12>();
let _d2 = gpioc.pc10.into_alternate::<12>();
let _d3 = gpioc.pc11.into_alternate::<12>();
let _ck = gpioc.pc12.into_alternate::<12>();
let _cmd = gpiod.pd2.into_alternate::<12>();

// DMA + SDMMC1
let mut sd = stm32h7xx_hal::sdmmc::Sdmmc::new(
    dp.SDMMC1,
    (/* d0..d3, ck, cmd pins */),
    ccdr.peripheral.SDMMC1,
    &ccdr.clocks,
);
sd.init_card(/* 4-bit, freq */).unwrap();

// Block device and FAT mount (adapter layer required)
let mut dev = rlvgl::platform::DiscoSdBlockDevice::new(sd);
  // TODO: mount with fatfs adapter and list /assets
```

### Resolución de problemas de SD

- Reloj: asegurar que el reloj del núcleo SDMMC1 se obtenga de PLL2 (ej., PLL2Q) a una
  velocidad razonable. Si es demasiado baja, la tarjeta puede agotar el tiempo de espera; si es demasiado alta, la inicialización falla.
- AF y pull-ups de GPIO: PC8..PC12 y PD2 deben ser AF12, muy alta velocidad; habilitar pull-ups
  donde sea necesario (externas de 47 kΩ típicamente presentes en las placas).
- Efectos de D-Cache: datos obsoletos o errores de CRC a menudo significan mantenimiento de caché faltante.
  El `DiscoSdBlockDevice` ya limpia/invalida; evitar búferes adicionales que el DMA
  no puede ver.
- Ancho de bus: iniciar en 1 bit, luego cambiar a 4 bits después de que la tarjeta informe soporte.
- Formato de tarjeta: usar MBR + FAT32. Evitar exFAT. Asegurar sectores lógicos de 512 bytes.
- Alimentación/cableado: verificar el rail de 3.3 V y la correcta inserción de la microSD. Volver a insertar la tarjeta.
- Controlador de núcleo ocupado: después de errores, ciclar completamente la alimentación de la placa para recuperar la
  máquina de estados de la tarjeta.

## Alimentación, Rendimiento y Robustez

- Mantenimiento de caché:
  - Asegurar la coherencia de D-Cache para los usuarios de DMA (SDMMC, DMA2D) durante el vaciado de la pantalla
    y las rutas de E/S de archivos.
- Manejo de errores y registro:
  - Agregar ganchos de registro ligeros (ej., ITM/SEGGER RTT o UART) para el arranque.
  - Mostrar errores significativos de la inicialización de I2C/pantalla para ayudar en el diagnóstico.

## Seguimientos del Generador BSP

- Entradas de regeneración:
  - Asegurar que rlvgl-creator siempre use la base de datos canónica de STM32
    (`rlvgl-chips-stm`) para la resolución de AF. No queda uso de `stm32_af.json`.
- Salida HAL/PAC:
  - Después de incrustar los activos de la base de datos canónica (`RLVGL_CHIP_SRC`), regenerar el
    BSP H747I-DISCO y verificar los AF (I2C4 en `PD12/PD13` → AF4, etc.).

## Pruebas y CI

- Verificaciones en el host:
  - Mantener `cargo fmt` / `clippy` en estado limpio con todas las combinaciones de características.
- Construcciones cruzadas:
  - Agregar un trabajo de CI para construir `rlvgl-stm32h747i-disco` para
    `thumbv7em-none-eabihf` utilizando el script del enlazador gestionado por `build.rs` del ejemplo.
- Pruebas de humo en el objetivo (manual/hardware):
  - Verificar la retroiluminación, el color de la pantalla limpia y los eventos táctiles que se
    repiten a través de UART.
  - Capturar una breve ejecución de demostración y comparar las secuencias de eventos esperadas.

## Hecho / Recientemente Implementado

- El Creator ahora resuelve funciones alternativas de la base de datos canónica de STM32;
  `--af` y `stm32_af.json` se eliminan de CLI/docs/scripts.
- El ejemplo obtiene una ruta para inicializar I2C4 a través de HAL y puentea a
  embedded-hal 1.0 para el controlador táctil (capa de compatibilidad temporal).
- Manejo de scripts del enlazador: el `build.rs` del espacio de trabajo organiza el
  `memory.x` del ejemplo en `OUT_DIR` y pasa `-Tmemory.x` al enlazador para
  objetivos embebidos.
- Se implementó el cableado de ejemplo para el reinicio del panel en `PG3`; el control de la
  retroiluminación funciona a través de un respaldo HAL-GPIO, con una ruta PWM TIM8
  habilitada por `backlight_pwm`.
- Andamio del dispositivo de bloque SD implementado para SDMMC1 con DMA e higiene de caché.

## Pulido restante de HAL/BSP y próximos pasos

- Plantilla HAL (H7):
  - Mantener `.set_speed(Speed::VeryHigh)` encadenado a `.into_alternate::<AF>()` en una sola
    declaración (evitar líneas que comienzan con `.` ).
  - No emitir importaciones por puerto (`gpioa::*`, etc.); solo `use stm32h7xx_hal::{gpio::Speed, pac, prelude::*};` y `use stm32h7xx_hal::rcc;`.
  - Asegurar la firma `configure_pins_hal(dp, ccdr)` para H7 y usar `dp.GPIOx.split(ccdr.peripheral.GPIOX)`.
- Regeneración de BSP: ejecutar `scripts/gen-example-bsp.sh` y verificar que el `examples/stm32h747i-disco/bsp/hal.rs` regenerado compile y pase `cargo fmt --check`.
- Pin-mux de ejemplo: cambiar al mux HAL (`bsp_hal::configure_pins_hal(&dp, &ccdr)`),
  eliminando el respaldo PAC temporal una vez que el archivo regenerado compile limpiamente.
- Resolución de AF: confirmar PD12/PD13 → I2C4 AF4 (base de datos canónica); eliminar el
  respaldo una vez que la base de datos proporcione AF para H747 de forma definitiva.
- Retroiluminación + reinicio:
  - Reemplazar la retroiluminación temporal GPIO con PWM HAL TIM8 (PJ6); agregar un pequeño adaptador
    `SetDutyCycle` de embedded-hal 1.0 sobre el canal PWM HAL.
  - Mantener el reinicio del panel en PG3 con retrasos compatibles; mover a HAL GPIO después de que el mux compile.
- CI/formato: volver a ejecutar `cargo fmt --all -- --check` y corregir los
  detalles de espacio en blanco o ajuste de línea de plantilla residuales para que los archivos
  generados permanezcan limpios con rustfmt.
```
