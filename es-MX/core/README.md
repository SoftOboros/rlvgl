```markdown
<!--
core/README.md - Vista general de las abstracciones centrales de rlvgl.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-core

Paquete: `rlvgl-core`.

Esta crate contiene las abstracciones de tiempo de ejecución que sustentan cada widget y
backend utilizado en **rlvgl**.

Piezas actualmente implementadas:

- Trait `Widget` que define las devoluciones de llamada de dibujo y eventos
- Árbol `WidgetNode` para composición jerárquica
- Enum `Event` para entrada básica
- Trait `Renderer` para dibujo independiente del objetivo
- Estructura `Style` con constructor para la apariencia del widget

Estas API son tempranas y evolucionarán a medida que más widgets y backends estén disponibles.
```
