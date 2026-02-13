```markdown
<!--
docs/TEST-TODO.md - rlvgl – Test TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl – Test TODO

Este archivo enumera el **flujo de trabajo de pruebas** para rlvgl. Cada entrada está ordenada aproximadamente en la secuencia en que debe abordarse, enumera sus **dependencias** ascendentes, ya sea por referencia a secciones de `docs/TODO.md` (`TODO#N`) o a pruebas anteriores, e indica si puede ser **completamente automatizada** (a través de `cargo test` impulsado por Codex, simulador sin interfaz gráfica, diff de imagen en CI, etc.) o requiere **verificación humana** (por ejemplo, aceptación visual en hardware real).

| ✔ | Orden | ID de Prueba | Descripción | Depende de | Automatización |
|---|-------|---------|-------------|-----------|------------|
| [x] | 1 | T-01 | **Pruebas unitarias del núcleo** – Invariantes del trait Widget, mutaciones del árbol, caída sin pánico | TODO#1 | Automatizado (Codex + `cargo test`) |
| [x] | 2 | T-02 | **Pruebas de envío de eventos** – Orden de captura/propagación, detención de la propagación | T-01 | Automatizado |
| [x] | 3 | T-03 | **Pruebas del constructor de estilo** – El patrón constructor produce las structs esperadas y los valores predeterminados | T-01 | Automatizado |
| [x] | 4 | T-04 | **Prueba de humo de Dummy DisplayDriver y Renderer** – Renderiza un marco de color sólido en un búfer RAM | TODO#3 | Automatizado (sin interfaz gráfica) |
| [x] | 5 | T-05 | **Pruebas de stub de InputDevice** – Marshalling de eventos de teclado/ratón | TODO#3 | Automatizado |
| [ ] | 6 | T-06 | **Integración SPI `st7789` smoke** en la placa STM32H7 NUCLEO | T-04, hardware | **Humano** (visual y osciloscopio) |
| [x] | 7 | T-07 | **Renderización dorada de widget de Nivel 1** – Diff de PNG de Label, Button, Container contra dorados | TODO#4, T-04 | Automatizado (simulador sin interfaz gráfica) |
| [x] | 8 | T-08 | **Prueba de estrés de diseño** – fuzzing de tamaños de contenedor y afirmación de ausencia de pánico / límites incorrectos | T-07 | Automatizado |
| [x] | 9 | T-09 | **Prueba de ventana del backend del simulador** – abre ventana SDL/pixels y renderiza el marco | TODO#5 | Automatizado (CI headless-X) |
| [x] | 10 | T-10 | **Dorados de widgets de Nivel 2** – Checkbox, Slider, Arc, List, Image | TODO#6, T-09 | Automatizado |
| [x] | 11 | T-11 | **Prueba de aplicación de tema** – Corrección de cascada de tema claro/oscuro | TODO#7, T-10 | Automatizado |
| [x] | 12 | T-12 | **Prueba de línea de tiempo de animación** – El fundido/deslizamiento produce fotogramas clave esperados (diff de hash a lo largo del tiempo) | TODO#7, T-11 | *Automatizado* (hash de fotograma) + **Humano** para suavidad |
| [ ] | 13 | T-13 | **Diff de demostración de paridad LVGL** – renderiza la demostración en C y rlvgl, diff de imagen perceptual ≤ ε | TODO#9, T-10 | Automatizado (CI) + **Humano** en diff > ε |
| [x] | 14 | T-14 | **Regresión de fuzzing de eventos** – toques/arrastres aleatorios contra widgets para 1k iteraciones con MIRI | T-07 | Automatizado |
| [x] | 15 | T-15 | **Regresión de tamaño incrustado** – `arm-none-eabi-size` + verificación de mapa de enlazador en CI | TODO#2 | Automatizado |
| [x] | 16 | T-16 | **Detección de memoria/fugas** con valgrind/asan bajo simulador | T-09 | Automatizado |
| [ ] | 17 | T-17 | **Benchmark de rendimiento** – FPS @ 240×320 en escritorio y placa H7 | T-09, T-06 | **Asistido por humanos** (medición de tiempo de hardware) |
| [x] | 18 | T-18 | **Prueba de compilación de fragmentos de código de documentación** – `doctest` todos los README/Ejemplos | TODO#8 | Automatizado |
| [x] | 19 | T-19 | **Enumeración de placas de proveedor** – consolida los crates de proveedor en una lista unificada | TODO-CHIP-SUPPORT | Automatizado |
| [x] | 20 | T-20 | **Manejo de errores de búsqueda de placa** – coincidencia exacta de nombre y errores útiles | T-19 | Automatizado |
| [x] | 21 | T-21 | **Menú desplegable de placas de UI** – la lista de selección se llena a partir de los crates de proveedor | T-19 | Automatizado |
| [x] | 22 | T-22 | **Cableado de entorno de Chip DB** – la compilación incrusta las definiciones de placa de `RLVGL_CHIP_SRC` | TODO-CHIP-SUPPORT | Automatizado |
| [x] | 23 | T-23 | **Publicar crates de chips de script** – el script de lanzamiento lista los crates de chipdb | T-22 | Automatizado |
| [x] | 24 | T-24 | **Pruebas de ingesta de MCU/IP de AFDB** – STM32 XML de muestra de ida y vuelta a través de superposiciones canónicas | TODO-CHIP-SUPPORT | Automatizado |
| [x] | 25 | T-25 | **Prueba de humo del constructor de catálogo de AFDB** – verifica las asignaciones de pines y los IOModes de GPIO en el catálogo generado | T-24 | Automatizado |
| [x] | 26 | T-26 | **Renderización de informe de AFDB** – la tabla de markdown lista los pines esperados y los modos GPIO | T-25 | Automatizado |

---

### Leyenda
- **Columna ✔** – marca `[x]` una vez que la prueba y sus criterios de aprobación se cumplen.
- **Automatizado** – puede ejecutarse en CI utilizando pruebas de Rust impulsadas por Codex, simulador sin interfaz gráfica o herramientas de diff perceptual.
- **Humano** – requiere ojos o mediciones físicas; intenta reducir el alcance solo a la aprobación donde sea inevitable.
- **Asistido por humanos** – métricas recolectadas automáticamente pero aún necesitan interpretación manual o configuración de hardware.

> A medida que se añaden nuevos elementos TODO, adjunta las pruebas correspondientes aquí, conéctalas a la cadena de dependencias y deja la casilla de verificación vacía hasta que la prueba esté completamente en verde en CI (o verificada por humanos donde sea aplicable).
```
