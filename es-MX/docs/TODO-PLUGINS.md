```markdown
<!--
docs/TODO-PLUGINS.md - rlvgl - Tareas pendientes del flujo de trabajo de plugins.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl - Tareas pendientes del flujo de trabajo de plugins

> **Propósito** Realizar un seguimiento de la portabilidad incremental de los complementos LVGL basados en C a crates de Rust para `rlvgl`. Las tareas se ordenan para respetar las dependencias técnicas, de modo que cada capa se base en la anterior.

---

## 🛠️ Instrucciones de preconfiguración de Codex

Antes de abordar las tareas pendientes de los plugins, Codex debe configurar el espacio de trabajo `rlvgl` para admitir el desarrollo modular de plugins mediante las características de Cargo.

### 1. Actualizar `Cargo.toml` con las características de los plugins

Añada lo siguiente a la sección `[features]`:

```toml
[features]
default = []

# Nivel 1
png = ["dep:png"]
jpeg = ["dep:jpeg-decoder"]
gif = ["dep:gif"]
qrcode = ["dep:qrcode"]
fontdue = ["dep:fontdue"]

# Nivel 2
lottie = ["dep:rlottie"]
canvas = ["dep:embedded-canvas"]
pinyin = []
fatfs = ["dep:fatfs-embedded"]
nes = ["dep:yane"]
```

Declare también las entradas `[dependencies]` con `optional = true`, por ejemplo:

```toml
[dependencies.png]
version = "*"
optional = true
```

### 2. Estructura del crate

Asegúrese de que cada plugin resida en su propio archivo `src/plugins/<nombre>.rs`:

```rust
#[cfg(feature = "png")]
pub mod png;
```

Luego en `lib.rs`:

```rust
#[cfg(feature = "png")]
pub use plugins::png;
```

### 3. Pruebas

Cada plugin debe tener:

- Pruebas unitarias `#[cfg(test)]` en su propio archivo.
- Pruebas de integración opcionales en `tests/plugins_png.rs`, etc.

Utilice los indicadores de características en las pruebas:

```rust
#[cfg(feature = "png")]
#[test]
fn test_png_decode() { /* … */ }
```

### 4. Fragmento de la matriz de CI

Soporte `cargo test --features gif,fontdue`, etc. Ejemplo de matriz de trabajo de CI:

```yaml
matrix:
  include:
    - features: "png jpeg gif"
    - features: "qrcode fontdue"
    - features: "lottie canvas"
```

---

## ⬛ Nivel 1 – Pipeline de medios y texto Core

*Componentes fundamentales necesarios antes
```
