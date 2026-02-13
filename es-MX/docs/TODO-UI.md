```markdown
<!--
docs/TODO-UI.md - rlvgl – Tareas pendientes del flujo de trabajo de la interfaz de usuario.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl – Tareas pendientes del flujo de trabajo de la interfaz de usuario

Este archivo rastrea las tareas para construir el crate de alto nivel `rlvgl-ui`.

## Fase 1 – Estilo y tema compatible con LVGL
- [x] Auditar las APIs de estilo de LVGL
- [x] StyleBuilder (padding, margin, bg, text, border, radius)
- [x] Ayudantes de Part/State
- [x] Estructuras de tokens (Spacing, Colors, Radii, Fonts)
- [x] Puente de tema heredado (material, mono)
- [x] Demostración + pruebas de CI
- [x] Etiquetar v0.1.0

## Fase 2 – rlvgl-ui Core
- [x] Ayudantes de diseño (HStack, VStack, Grid, Box)
- [x] Ganchos de eventos (on_click, on_change)
- [x] Integración de fuente de iconos
- [x] Macro DSL opcional (view!) detrás de un feature flag
- [x] Publicar rlvgl-ui v0.1

## Fase 3 – Componentes inspirados en Chakra
 - [x] Button / IconButton
 - [x] Text / Heading
 - [x] Input / Textarea
 - [x] Checkbox
 - [x] Switch
- [x] Radio
- [x] Badge / Tag / Alert
 - [x] Modal / Drawer / Toast
- [ ] Aplicación de demostración estilo Storybook
- [ ] Lanzar v0.2 y bosquejo 1.0

---

Licencia MIT: MIT.
```
