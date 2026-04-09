```markdown
<!--
rlvgl-decomp/README.md - Decodificador/codificador RLE para el formato splash de rlvgl.
-->

# rlvgl-decomp

Utilidades principales de formato de imagen comprimida para rlvgl.

Esta crate proporciona un formato compacto de longitud de ejecución (run-length) con
una paleta y códigos de escape de píxeles en línea, además de un codificador
básico que construye una paleta y emite un flujo de repetición corto/largo.
Ambos operan en fotogramas RGBA y convierten a/desde RGB565 internamente para
coincidir con las pipelines de visualización embebidas.

Características:
- Compatible con no-std (usa `alloc`).
- Decodificador para el formato RLE (paleta + flujo de bytes → RGBA).
- Codificador de RGBA → paleta (RGB565) + flujo de bytes usando repetición/diccionario.

El formato es un punto de partida para herramientas de creación para convertir
entradas (por ejemplo, fotogramas PNG/APNG/Lottie) en una representación
compacta consumible por rlvgl.
```
