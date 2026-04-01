```markdown
<!--
docs/TODO-RENDERING.md - rlvgl – Tareas pendientes del flujo de trabajo de renderizado.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl – Tareas pendientes del flujo de trabajo de renderizado

Este archivo rastrea las tareas para mejorar el pipeline de renderizado de modo que los widgets puedan dibujar múltiples capas con mezcla alfa. Todos los valores de color deben contener datos RGBA desde el origen hasta la visualización; si cada capa es transparente en un píxel, el color de la capa más baja permanece visible.

## Renderizado con soporte para Alpha
- [x] Extender `Colour` de RGB a RGBA para que los widgets puedan expresar opacidad.
- [ ] Agregar métodos de mezcla conscientes del alfa a `Renderer` y actualizar los backends.
- [ ] Definir la semántica de capas/composición de widgets para que las capas superiores se mezclen sobre las inferiores.
- [ ] Propagar colores RGBA a través de las API de estilo y relleno en widgets y backends.

---

*Última actualización 2025-08-06*
```
