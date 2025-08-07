# rlvgl-ui ─ Unified Documentation
*(Copy-paste this single file into `ui/README.md` or anywhere you like.)*

---

## 1 ▸ Overview

**rlvgl-ui** is a second-layer crate that sits atop the low-level `rlvgl` bindings
(and therefore the C-based **LVGL** engine).

It offers a **Chakra / React-inspired API**—themes, tokens, fluent styles, and
composable components—without sacrificing the raw speed and tiny footprint that
make LVGL the go-to GUI for micro-controllers and small MPUs.

┌─────────────┐ Your app (Button::new().on_click(save))
├─────────────┤ rlvgl-ui (Theme, Style, VStack …)
├─────────────┤ rlvgl (safe Rust LVGL wrappers)
├─────────────┤ lvgl-sys (raw C FFI)
└─────────────┘

### Why another layer?

| Benefit        | Details                                                             |
|----------------|---------------------------------------------------------------------|
| Familiarity    | React / Chakra devs feel at home.                                   |
| Productivity   | `Style::new().bg(...)` replaces dozens of `lv_obj_set_style_*()` calls. |
| Interoperable  | 100 % compatible with LVGL themes & styles; C and Rust can mix.     |
| Tiny Footprint | Adds ergonomics, **not** a JS engine or GC.                         |

---

## 2 ▸ Quick Start

#### `Cargo.toml`
```toml
[dependencies]
rlvgl     = "0.2"
rlvgl-ui  = { path = "ui" }   # local path while hacking
```

Minimal code

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

Build & run

Desktop simulator:

```
cargo run --example demo -p rlvgl-ui
```

MCU target (e.g. STM32-H723):

```
cargo build --release --target thumbv7em-none-eabihf -p rlvgl-ui
```

## 3 ▸ Roadmap / TODO

### Phase 1 · LVGL-Compatible Style & Theme
- [x] Audit LVGL style APIs
- [x] StyleBuilder (padding, margin, bg, text, border, radius)
- [x] Part/State helpers
- [x] Token structs (Spacing, Colors, Radii, Fonts)
- [x] Legacy theme bridge (material, mono)
- [x] Demo + CI tests
- [x] Tag v0.1.0

### Phase 2 · rlvgl-ui Core
- [x] Layout helpers (HStack, VStack, Grid, Box)
- [x] Event hooks (on_click, on_change)
- [x] Icon font integration
- [x] Optional macro DSL (view!) behind feature flag
- [x] Publish rlvgl-ui v0.1

### Phase 3 · Chakra-Inspired Components
 - [x] Button / IconButton
 - [x] Text / Heading
 - [x] Input / Textarea
 - [x] Checkbox
 - [x] Switch
 - [x] Radio
 - [x] Badge / Tag / Alert
 - [x] Modal / Drawer / Toast
 - [ ] Storybook-style demo app
 - [ ] Release v0.2 and draft 1.0

## 4 ▸ Agent Specification (temperature = 0 %)

Deterministic instructions for any LLM or tool generating or refactoring code
inside ui/.
Modify files only within ui/ unless explicitly instructed.
Preserve public API signatures unless version number is bumped.
All generated styles must compile to valid `lv_style_t` data.
Token namespaces are fixed: spacing, colors, radii, fonts.
Maximum source-line length: 100 columns.
MIT-license header: MIT / Apache-2.0.

## 5 ▸ Example (ui/examples/demo.rs)

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

## 6 ▸ License

MIT-licensed: MIT.

“Tiny screens deserve great UX, too.”
