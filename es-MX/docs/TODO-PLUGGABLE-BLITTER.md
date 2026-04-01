<!--
docs/TODO-PLUGGABLE-BLITTER.md - Epic: Backends de Renderizado/Pantalla Conectables (CPU, DMA2D, winit/wgpu).
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Épica: Backends de Renderizado/Pantalla Conectables (CPU, DMA2D, winit/wgpu)

**Descripción**: Introduce un rasgo de estrategia `Blitter` y múltiples implementaciones (CPU de respaldo, STM32H7 DMA2D, wgpu de escritorio). Conecta estos bajo `platform/` para que el mismo código de widget/renderizado apunte a embebidos y escritorio. Agrega LTDC/DSI + OTM8009A (DISCO) y tacto FT5336. Actualiza el simulador para usar `winit + wgpu` (ventana + GPU) para mayor velocidad.  
**Resultado**: Rutas de vaciado aceleradas por hardware en H7; simulador de alta velocidad de fotogramas; pruebas unificadas.

---

## A) Abstracción de Blitter (plataforma)

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Definir rasgo `Blitter`: `caps()`, `fill()`, `blit()`, `blend()`, soporte PFC | `bitflags` (caps) | Tipos de rectángulos y superficies viven en `platform::blit`. |
| [x] | Agregar `Surface` (buf/stride/fmt/w,h) + enumeración `PixelFmt` | ninguna | Incluir ARGB8888, RGB565, L8/A8/A4. |
| [x] | Agregar `BlitPlanner` para agrupar rectángulos sucios por fotograma | ninguna | Opcional: unir rectángulos adyacentes. |
| [x] | Enlazar a través del renderizador → blitter (sin fuga de API a widgets) | renderizador de plataforma | El renderizador posee un `&mut dyn Blitter`. |

---

## B) Blitter de Respaldo de CPU

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Implementar `CpuBlitter` (bucles escalares) | ninguna | Base de corrección, usada en pruebas. |
| [x] | Rutas rápidas para formatos comunes (ARGB8888→RGB565, rellenos) | ninguna | Considerar `bytemuck` para conversiones. |
| [x] | Pruebas unitarias (buffers dorados) | `proptest` opcional | Reutilizar las mismas pruebas en todos los backends. |

---

## C) Blitter DMA2D de STM32H7 (“GPU”)

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Crear `Dma2dBlitter` con acceso a registros PAC | `stm32h7` PAC, `cortex-m` | HAL carece de DMA2D completo; usar PAC. |
| [x] | Inicialización: reloj, configuración de capa frontal/trasera, desplazamiento de línea | PAC | Mantener el wrapper seguro; sin `unsafe` en la API. |
| [x] | Implementar R2M (relleno) | PAC | Bloqueando primero; agregar IRQ después. |
| [x] | Implementar M2M/PFC (copia + conversión) | PAC | Ruta común ARGB8888→RGB565. |
| [x] | Implementar mezcla M2M (FG sobre BG, alfa constante/por píxel) | PAC | Asunción de alfa directo; documentarlo. |
| [x] | Opcional: no bloqueante con interrupción/finalización | EXTI/IRQ | Encolar operaciones; barrera antes de VSYNC. |
| [ ] | Reutilizar pruebas de CPU para afirmar píxeles idénticos | `std` test vía construcción en host | Usar imágenes de prueba pequeñas, recortes. |

---

## D) Pantalla STM32H747I‑DISCO (LTDC/DSI + OTM8009A)

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Configurar relojes para LTDC/DSI (config RCC) | `stm32h7xx-hal` (RCC) | Coincidir con la temporización del panel. |
| [x] | SDRAM (FMC) si FB en RAM externa | HAL FMC o PAC | AXI SRAM está bien para pruebas pequeñas. |
| [x] | Host DSI + secuencia de inicialización OTM8009A (modo de video) | PAC | Portar de C BSP; factorizar `otm8009a.rs`. |
| [x] | Configuración de capa LTDC (dirección FB, stride, fmt) | PAC | Iniciar FB RGB565 para ahorrar RAM. |
| [x] | PWM de retroiluminación + GPIO de RESET de panel | HAL TIM/GPIO | Línea TE opcional para vsync. |
| [x] | Conexión `Stm32h747iDiscoDisplay<B: Blitter>` | secciones A/C | Componer el blitter seleccionado. |
| [x] | Bandera de característica: `stm32h747i_disco` | Características de Cargo | Bloquear dependencias sin std/manejador de pánico. |

---

## E) Tacto FT5336 (I²C + EXTI)

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Inicialización I²C @ 400 kHz | `stm32h7xx-hal` I2C | Usar pines de la placa. |
| [x] | EXTI en línea INT (opcional) | HAL EXTI | O sondear en `poll()`. |
| [x] | Driver FT5336 mínimo: leer puntos | ninguna | Convertir a `Event` (abajo/mover/arriba). |
| [x] | Integración `Stm32h747iDiscoInput` | entrada de plataforma | Coordinar configuración de volteo/rotación. |

---

## F) Simulador de Escritorio: Backend **winit + wgpu**

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Reemplazar/minimizar el uso de `pixels/minifb` | `winit`, `wgpu` | Ventana `winit` + cadena de intercambio `wgpu`. |
| [x] | `WgpuBlitter` implementando `Blitter` | `wgpu` | Usar render pass + quads texturizados o cómputo. |
| [x] | Subir mosaico/rectángulo a textura; blit/blend en shader | `wgpu` | Texturas actualizadas y mezcladas mediante pipelines de renderizado. |
| [x] | Presentar @ vsync; mapear teclado/ratón → `InputDevice` | `winit` | Escalado de DPI; cadena de intercambio sRGB. |
| [x] | Modo sin cabeza para volcar PNGs para CI | `image` | Pruebas de regresión de imágenes doradas. |

---

## G) Ejemplo de Panel SPI (ST7789) para Probar Portabilidad

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Driver `st7789` a través de `embedded-hal` | `embedded-hal` | Reutilizar `CpuBlitter`. |
| [ ] | Ruta de vaciado DMA SPI | HAL DMA | Opcional: líneas de doble búfer. |

---

## H) Integración y CI

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Matriz de características de Cargo (`cpu`, `dma2d`, `wgpu`) | Cargo | Hacer que los backends sean intercambiables. |
| [ ] | Trabajos de CI: pruebas de host + wgpu offscreen + informe de tamaño | Acciones de GitHub | Mantener las comprobaciones de tamaño actuales. |
| [ ] | Ejemplo: `examples/sim` usa `wgpu` | F) | Atajos de teclado: alternar depuración de rectángulos sucios. |
| [ ] | Ejemplo: `examples/STM32H747I-DISCO` usa DMA2D | C/D/E | Comparte código de aplicación con sim (refactorizar). |

---

## I) Documentación y Diferencias

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | `#![doc = include_str!(…)]` para APIs públicas | ninguna | Refleja el estilo del proyecto. |
| [ ] | Documento para desarrolladores: "Eligiendo un blitter/backend" | ninguna | Cuándo elegir cuál, compensaciones de memoria. |
| [ ] | Arnés de diferencias de imagen (salida de sim vs dorado) | `image`, `assert_cmd` | Delta RGBA con umbral. |

---

## J) Plugins y Widgets – Integración de Blitter

| Hecho | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Integrar el rasterizador de texto `fontdue` en `BlitterRenderer` | `fontdue` | Almacenar en caché los glifos como `Surface`s; soportar rutas CPU/WGPU/DMA2D. |
| [x] | Conectar decodificadores de imagen (`png`, `jpeg`, `gif`, `apng`) para producir superficies de blitter | `png`, `jpeg`, `gif`, `apng` | Decodificar a `Surface` y llamar a `blit()`/`blend()`; manejar fotogramas de animación. |
| [x] | Renderizar `QrWidget` a través del pipeline del blitter | `qrcode` | Generar mapa de bits QR, subir como `Surface`, evitar escrituras directas en el framebuffer. |
| [x] | Unir fotogramas `rlottie` a superficies de blitter | `rlottie` | Convertir fotogramas vectoriales a `Surface`; permitir aceleración de GPU. |
| [x] | Tratar los buffers `CanvasWidget` como superficies de blitter | `embedded-canvas` | Vaciar incrementalmente las regiones sucias a través del blitter. |
| [x] | Enrutar widgets de nivel superior (IME pinyin, selector de archivos FATFS, demo NES) a través de la pila de canvas/blitter | `pinyin`, `fatfs-embedded`, `yane` | Asegurar que sus rutas de renderizado permanezcan agnósticas al backend. |
