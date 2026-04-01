```markdown
<!--
ui/README.md - Unified documentation for rlvgl-ui.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-ui ─ Documentación Unificada
Paquete: `rlvgl-ui`
*(Copia y pega este único archivo en `ui/README.md` o donde prefieras.)*

---

## 1 ▸ Resumen

**rlvgl-ui** es un crate de segunda capa que se asienta sobre los bindings de bajo nivel de `rlvgl`
(y por lo tanto, el motor **LVGL** basado en C).

Ofrece una **API inspirada en Chakra / React**—temas, tokens, estilos fluidos y
componentes componibles—sin sacrificar la velocidad pura y la pequeña huella que
hacen de LVGL la GUI preferida para microcontroladores y MPUs pequeños.

┌─────────────┐ Tu aplicación (Button::new().on_click(save))
├─────────────┤ rlvgl-ui (Theme, Style, VStack …)
├─────────────┤ rlvgl (envoltorios seguros de Rust para LVGL)
├─────────────┤ lvgl-sys (FFI de C puro)
└─────────────┘

### ¿Por qué otra capa?

| Beneficio      | Detalles                                                            |
|----------------|---------------------------------------------------------------------|
| Familiaridad   | Los desarrolladores de React / Chakra se sienten como en casa.     |
| Productividad  | `Style::new().bg(...)` reemplaza docenas de llamadas a `lv_obj_set_style_*()`. |
| Interoperable  | 100 % compatible con temas y estilos de LVGL; C y Rust pueden mezclarse. |
| Huella Pequeña | Añade ergonomía, **no** un motor JS o GC.                        |

---

## 2 ▸ Inicio Rápido

#### `Cargo.toml`
```toml
[dependencies]
rlvgl     = "0.2"
rlvgl-ui  = { path = "ui" }   # local path while hacking
```

Código mínimo

```rust
use rlvgl_ui::{Theme, Style, Button, VStack};

fn ui() {
    let theme = Theme::material_light();
    theme.apply_global();               // push tokens to LVGL

    VStack::new()
        .spacing(theme.spacing.md)
        .child(
            Button::new("Save")
                .icon("save")           // built-in icon font
                .style(
                    Style::new()
                        .bg(theme.colors.primary)
                        .radius(theme.radii.md)
                )
                .on_click(|| { println!("Saved!"); })
        )
        .mount(lv_scr_act());
}
```

Compilar y ejecutar

Simulador de escritorio:

```
cargo run --example demo -p rlvgl-ui
```

Objetivo MCU (ej. STM32-H723):

```
cargo build --release --target thumbv7em-none-eabihf -p rlvgl-ui
```

## 3 ▸ Hoja de Ruta / PENDIENTE

### Fase 1 · Estilo y Tema Compatibles con LVGL
- [x] Auditoría de APIs de estilo de LVGL
- [x] StyleBuilder (relleno, margen, fondo, texto, borde, radio)
- [x] Ayudantes de Parte/Estado
- [x] Estructuras de Tokens (Espaciado, Colores, Radios, Fuentes)
- [x] Puente de tema heredado (material, mono)
- [x] Demo + pruebas CI
- [x] Etiquetar v0.1.0

### Fase 2 · Núcleo de rlvgl-ui
- [x] Ayudantes de diseño (HStack, VStack, Grid, Box)
- [x] Ganchos de eventos (on_click, on_change)
- [x] Integración de fuentes de iconos
- [x] DSL de macro opcional (view!) detrás de un feature flag
- [x] Publicar rlvgl-ui v0.1

### Fase 3 · Componentes Inspirados en Chakra
 - [x] Botón / IconButton
 - [x] Texto / Encabezado
 - [x] Input / Textarea
 - [x] Casilla de verificación
 - [x] Interruptor
 - [x] Radio
 - [x] Insignia / Etiqueta / Alerta
 - [x] Modal / Cajón / Tostada
 - [ ] Aplicación de demostración estilo Storybook
 - [ ] Lanzar v0.2 y el borrador 1.0

## 4 ▸ Especificación del Agente (temperatura = 0 %)

Instrucciones determinísticas para cualquier LLM o herramienta que genere o refactorice código
dentro de ui/.
Modificar archivos solo dentro de ui/ a menos que se indique explícitamente.
Conservar las firmas de la API pública a menos que se incremente el número de versión.
Todos los estilos generados deben compilarse a datos `lv_style_t` válidos.
Los espacios de nombres de los tokens son fijos: espaciado, colores, radios, fuentes.
Longitud máxima de la línea de código fuente: 100 columnas.
Encabezado de licencia MIT: MIT / Apache-2.0.

## 5 ▸ Ejemplo (ui/examples/demo.rs)

```rust
use rlvgl_ui::{Theme, Style, Button, VStack};

pub fn build() {
    let theme = Theme::material_light();
    theme.apply_global();

    VStack::new()
        .spacing(theme.spacing.md)
        .child(
            Button::new("Save")
                .icon("save")
                .style(
                    Style::new()
                        .bg(theme.colors.primary)
                        .radius(theme.radii.md)
                )
                .on_click(|| { println!("Saved!"); })
        )
        .mount(lv_scr_act());
}
```

## 6 ▸ Licencia

Licencia MIT: MIT.

“Las pantallas pequeñas también merecen una gran experiencia de usuario.”
```
