<!--
TODO-CREATOR-BSP.md - Lista de tareas para el generador BSP en rlvgl-creator.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Tareas Pendientes - Creador BSP

Este archivo rastrea el trabajo restante para el generador de paquetes de soporte
de placa (`BSP`) de `rlvgl-creator`. El generador opera en dos etapas:

1. **Importa** archivos de configuración del proveedor (`.ioc`, `.mex`, etc.) a un
   IR YAML pequeño y neutral del proveedor que describe relojes, grupos de pines,
   DMA, interrupciones y parámetros de periféricos.
2. **Genera** código BSP de Rust renderizando plantillas MiniJinja contra el IR.

## Tareas

- [x] Implementar script de Python en `tools/afdb/` para construir una base de datos
      JSON de funciones alternativas para STM32.
- [x] Desarrollar el adaptador STM32 CubeMX `.ioc` para cubrir la configuración de PLL
      y el reloj del kernel.
- [x] Añadir plantillas a nivel de clase para la instanciación de USART, SPI e I2C
      utilizando números de instancia derivados de los nombres de los periféricos.
- [x] Denegar la configuración de pines reservados (SWD: `PA13`, `PA14`) a menos que
      se proporcione una anulación explícita.
- [ ] Proporcionar adaptadores para proveedores adicionales:
  - [x] Espressif
  - [x] Microchip
  - [x] Nordic
  - [x] NXP
  - [x] Renesas
  - [x] RP2040
  - [x] Silicon Labs
  - [x] TI
- [x] Documentar los ayudantes de plantilla y el esquema IR para que los usuarios
      puedan suministrar plantillas personalizadas.
- [x] Añadir pruebas unitarias que capturen el IR y la salida generada para
      proyectos de proveedores de ejemplo.
- [x] Dividir el código generado en funciones de ayuda `enable_gpio_clocks`,
      `configure_pins` y `enable_peripherals`.
- [x] Colapsar las escrituras RCC por registro para emitir una única llamada de
      modificación OR'd por bus.
- [x] Configurar los pines I2C como drenaje abierto con resistencias pull-up en las
      plantillas PAC.
- [x] Emitir configuraciones de muy alta velocidad para pines ULPI, SDMMC y SPI.
- [x] Limitar los bloques `unsafe` a las líneas `w.bits(...)` en el código generado.
- [x] Seleccionar los nombres de bus RCC por familia de MCU al habilitar los relojes.
- [x] Anteponer encabezados SPDX y de procedencia a todos los archivos generados.
- [x] Proporcionar ganchos de desinicialización opcionales que controlen los relojes
      y liberen los pines.
- [x] Permitir alternadores del generador como `--grouped-writes`, `--emit-hal`,
      `--emit-pac`, `--one-file`, `--per-peripheral` y `--with-deinit`.
- [x] Añadir atributos de higiene en tiempo de compilación (`#![allow(non_snake_case)]`
      y `#[allow(clippy::too_many_arguments)]`) al pegamento generado.
- [x] Proteger los módulos por periférico con características de Cargo.
- [x] Deshabilitar los relojes DMA y las interrupciones durante la desinicialización.
- [x] Restablecer los registros de configuración de DMA y borrar los indicadores de
      interrupción para streams y canales.
- [x] Emitir declaraciones `mod` padre con funciones protegidas para diseños
      por periférico.
- [x] Integrar los controladores BDMA y MDMA en la limpieza de DMA.
- [x] Refinar la habilitación del reloj periférico en las subfamilias STM32 restantes.
- [x] Demostrar la generación de BSP consciente del bus en ejemplos de placas
      adicionales.
- [x] Ampliar la cobertura de BDMA/MDMA para variantes STM32 adicionales (F0, F1, F2, F3, U5, WB, WL).
- [x] Expandir las demostraciones conscientes del bus a más placas de descubrimiento y
      evaluación, incluyendo H573I-DISCO y U599I-EVAL.
- [x] Pulir la documentación del generador con ejemplos de configuración avanzada
      y guías.
- [x] Mapear registros RCC específicos de periféricos en las familias STM32 restantes.
- [x] Cubrir restablecimientos de registros DMA adicionales y casos extremos.
- [x] Documentar los casos extremos restantes y los inconvenientes en la referencia de CLI.

## Notas

- No se deben mantener tablas por chip; todos los datos de instancia se derivan
  programáticamente de los metadatos del proveedor.
- Mantener el IR pequeño y alinear las clases con las características de
  `embedded-hal` para mantener la neutralidad del proveedor.
