```markdown
<!--
docs/TODO-DECOMP.md - Plan de trabajo para rlvgl-decomp (paleta + códec RLE)
-->

<p align="centre">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-decomp Tareas Pendientes

Este documento rastrea el trabajo pendiente para la crate `rlvgl-decomp`: un
formato de imagen compacto de paleta + RLE con repeticiones cortas/largas y píxeles
en línea de escape. La crate apunta a `no_std` con `alloc` y opera en
fotogramas RGBA, convirtiendo a/desde RGB565 internamente para coincidir con
las tuberías de visualización embebidas.

## Objetivos

- Proporcionar un formato de imagen comprimido estable y documentado para los activos de rlvgl.
- Decodificar a RGBA rápidamente en dispositivos embebidos con una memoria mínima.
- Codificar entradas RGBA de manera eficiente para herramientas de creación; soportar fotogramas individuales y secuencias.
- Mantenerse `no_std`; soportar solo `alloc`.

## Formato (Recapitulación)

- Paleta: hasta `MAX_PALETTE` entradas RGB565 (por defecto 192) derivadas del histograma del fotograma.
- Bytes del flujo:
  - `0xFF` (en línea simple): los siguientes 2 bytes RGB565; emitir una vez.
  - `0xFE` (en línea doble): los siguientes 2 bytes RGB565; emitir dos veces.
  - `0xFD` (repetición larga): repetir el color del índice de paleta más reciente para `61 + next_byte` píxeles (hasta 316).
  - `0..(palette_len-1)`: índice de paleta; emitir una vez; establece el índice reciente.
  - `(palette_len)..(palette_len+60)`: repetición corta; emitir el índice reciente `(byte - palette_len + 1)` veces.

Notas:
- El codificador limita la paleta para que los códigos de repetición corta nunca colisionen con `0xFD`..`0xFF`.
- El decodificador valida longitudes y límites de paleta; devuelve `Error::Truncated`/`SizeMismatch`.

## Elementos de trabajo

- Pulido del decodificador
  - [ ] Añadir API de decodificación en streaming (fila por fila) para limitar el pico de memoria.
  - [ ] Exponer la opción de salida RGB565 para evitar la expansión RGBA en embebidos.
  - [ ] Validar desbordamiento/casos límite (paleta vacía, imágenes de tamaño cero).

- Mejoras del codificador
  - [ ] Estrategias de selección de paleta: corte mediano / k-medias de respaldo para mejorar la calidad.
  - [ ] Detección de ejecución a través de filas (permitir que las ejecuciones continúen sobre los límites de la línea de exploración opcionalmente).
  - [ ] Estrategia mixta para colores no paleta: pequeña paleta local vs heurística de píxeles en línea.
  - [ ] Ajustar los umbrales de repetición larga/corta; dividir automáticamente las ejecuciones muy largas.
  - [ ] Añadir codificación consciente de la región (azulejos) para una mejor reutilización local en imágenes complejas.

- Compresión basada en diccionario (fase siguiente)
  - [ ] Construir diccionario de primer orden: tuplas frecuentes de 2 a 4 píxeles (RGB565) → códigos.
  - [ ] Extender el flujo con la sección del diccionario y las claves de escape (reservar por debajo de `0xF0`).
  - [ ] Heurística del codificador para elegir RLE vs. aciertos de diccionario por segmento.
  - [ ] Bandera de compatibilidad hacia atrás en la cabecera para señalar la presencia del diccionario.

- Contenedor/cabecera
  - [ ] Definir una cabecera mínima: magia, versión, ancho, alto, banderas de formato, longitud de paleta.
  - [ ] Agrupar paleta + flujo (+ diccionario opcional) como un solo blob.
  - [ ] Little-endian, cabecera de tamaño fijo para un fácil análisis.

- Integración del creador
  - [ ] Añadir subcomando CLI de rlvgl-creator: `creator assets encode --format rle`.
  - [ ] Soportar secuencias (APNG/Lottie): emitir fotogramas numerados o un simple contenedor de múltiples fotogramas.
  - [ ] Opción para el objetivo RGB565 directamente para omitir el viaje de ida y vuelta de RGBA.

- Pruebas y CI
  - [ ] Pruebas unitarias: patrones pequeños de ida y vuelta (sólido, tablero de ajedrez, gradientes, ejecuciones largas).
  - [ ] Decodificación de flujo fuzz (longitudes, claves, límites de paleta) bajo `std`.
  - [ ] Muestras doradas bajo `tests/` con imágenes de fixture.
  - [ ] Benchmarks (host): rendimiento de codificación/decodificación y tamaño vs. PNG (cordura).

- Rendimiento y memoria
  - [ ] Evitar asignaciones intermedias durante la decodificación (proporcionar API de búfer propiedad del llamador).
  - [ ] Ruta SIMD opcional para conversiones RGBA<->RGB565 en compilaciones de host.
  - [ ] Codificador basado en iteradores para reducir los histogramas temporales para fotogramas grandes.

- Documentación
  - [ ] Documentos de la API pública con ejemplos.
  - [ ] Página de especificación de formato (estable), incluir diagramas de bytes.
  - [ ] Documentos de uso del creador y solución de problemas (bandas de color, tamaño de paleta, umbrales).

## Deseables

- [ ] Controles de cuantificación de paleta con pérdida (opciones de dither, límite de tamaño de paleta).
- [ ] Codificación de azulejos/franjas para acelerar los redibujados parciales.
- [ ] Codificación delta opcional por fotograma para secuencias.

## Aceptación

- Decodificador: pasa las pruebas unitarias y decodifica los activos de muestra sin errores.
- Codificador: produce blobs más pequeños que RGBA en activos de interfaz de usuario típicos; tamaño de paleta configurable.
- Creador: puede ingerir PNG/APNG/Lottie y emitir el contenedor; documentos básicos en `docs/`.
- CI: compila de forma estable; `cargo fmt`, `clippy` limpio; comprobador de enlaces ok.
```
