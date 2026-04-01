<!--
docs/CREATOR-TEMPLATES.md - rlvgl-creator Templates and Hooks.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Plantillas y Ganchos de rlvgl-creator

Documentación para desarrolladores que describe cómo el creador utiliza plantillas Tera incrustadas para andamiar "asset crates" y dónde extender el pipeline de conversión.

## Plantillas
El comando `scaffold` construye un "asset crate" usando plantillas Minjinja que están incrustadas como constantes de cadena en [`src/bin/creator/scaffold.rs`](../src/bin/creator/scaffold.rs). Estas plantillas cubren archivos como `Cargo.toml`, `lib.rs`, `build.rs` y `README.md`. Modifique las constantes correspondientes para cambiar el diseño del "crate" generado o añadir nuevos archivos.

## Ganchos de Pipeline
La lógica de conversión reside en archivos Rust modulares como [`convert.rs`](../src/bin/creator/convert.rs), [`fonts.rs`](../src/bin/creator/fonts.rs) y [`lottie.rs`](../src/bin/creator/lottie.rs). Las nuevas etapas del pipeline pueden engancharse al proceso añadiendo un módulo e invocándolo desde `convert.rs`. Cada paso recibe metadatos del activo y puede emitir salidas a `.cache` para su reutilización.
