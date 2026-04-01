```markdown
<!--
docs/STM32H747I-DISCO.md - Notas de hardware STM32H747I-DISCO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Notas de hardware STM32H747I-DISCO

Este documento contiene los mapeos de pines y detalles de configuración de periféricos para usar la placa STM32H747I-DISCO con rlvgl.

## Pantalla

- Pantalla TFT de 4" 800×480 controlada por el host DSI en modo de video
- Controlador OTM8009A configurado para píxeles RGB888 y orientación horizontal
- `BSP_LCD_Init()` configura los relojes, LTDC y DSI para activar el panel

## Táctil

- Controlador capacitivo FT5336 en I2C4 en la dirección de 7 bits 0x38 (8 bits 0x70)
- I2C4 SCL: PD12, SDA: PD13 (AF4), interrupción: PK7
- Frecuencia de bus recomendada de 400 kHz (el asistente HAL lo configura); admite dos
  puntos táctiles concurrentes

## Tarjeta SD

La ranura microSD integrada está conectada al periférico SDMMC1 en modo de 4 bits
de ancho.

### Asignaciones de pines de CubeMX

| Pin  | Función      | Función Alternativa |
| ---- | ------------ | ------------------- |
| PC8  | SDMMC1_D0    | AF12                |
| PC9  | SDMMC1_D1    | AF12                |
| PC10 | SDMMC1_D2    | AF12                |
| PC11 | SDMMC1_D3    | AF12                |
| PC12 | SDMMC1_CK    | AF12                |
| PD2  | SDMMC1_CMD   | AF12                |

Habilite los relojes GPIOC y GPIOD y configure todos los pines a muy alta velocidad con
pull-ups internos. SDMMC1 debe obtener su reloj de kernel del PLL2 con una
salida de 200 MHz. Se recomiendan las transmisiones DMA2 3 (RX) y 6 (TX) utilizando el canal 4
para transferencias de datos.

## Luz de fondo y Reinicio

- La luz de fondo utiliza TIM8 (por ejemplo, CH1/CH2) en `PJ6` (complementario opcional `CH2N`
  en `PJ7`) para el control de brillo PWM. Para una puesta en marcha temprana, un fallback GPIO alto/bajo
  en `PJ6` es aceptable.
- El reinicio del panel está mapeado a `PG3` (LCD_RESET). Aplique retrasos conformes a la hoja de datos
  entre el reinicio bajo/alto y la inicialización del enlace DSI.
```
