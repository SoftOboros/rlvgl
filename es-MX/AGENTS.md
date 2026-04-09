<!--
AGENTS.md - Guía para colaboradores y convenciones de proyecto.
-->
<p align="center">
  <img src="rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl

Una reimplementación modular e idiomática en Rust de LVGL (Light and Versatile Graphics Library).

rlvgl preserva el paradigma de interfaz de usuario basado en widgets de LVGL, eliminando la gestión de memoria de estilo C insegura y el estado global. Esta librería está estructurada para soportar entornos no_std, objetivos embebidos (por ejemplo, STM32H7) y backends de simulador para prototipado rápido.

La versión en C de LVGL se incluye como un submódulo de git para referencia y extracción de vectores de prueba, pero no se enlaza ni se compila en esta librería.

## Objetivos

*   Preservar la arquitectura y el sistema de diseño de LVGL
*   Reemplazar el manejo de memoria en C con la propiedad idiomática de Rust
*   Soportar la actualización de pantalla y la entrada embebidas a través de embedded-hal
*   Habilitar la jerarquía de widgets, estilos y eventos utilizando traits de Rust
*   Usar crates de Rust existentes siempre que sea posible (por ejemplo, embedded-graphics, heapless, tinybmp)

## Características

*   no_std + soporte de asignador
*   Diseño modular basado en componentes (core, widgets, platform)
*   Simulable a través de una bandera de característica habilitada para std
*   Backends de pantalla y entrada conectables

## Estructura del Proyecto

*   `core/` – Trait base de Widget, diseño, despacho de eventos
*   `widgets/` – Reimplementaciones nativas en Rust de widgets de LVGL
*   `platform/` – Traits de pantalla/entrada y adaptadores HAL
*   `lvgl/` – Submódulo C (solo referencia)

## Estado

Tal como fue construido. Consulte `./docs/TODO.md` para ver el progreso componente por componente.

## Directrices del Generador BSP

La herramienta `rlvgl-creator` convierte archivos de configuración del proveedor en un
IR neutral del proveedor y renderiza código BSP de Rust a través de plantillas MiniJinja.

*   Evite las tablas por chip. Las reglas de clase de periféricos deben ser reutilizables en todas las
    instancias y proveedores.
*   Los números de función alternativos se resuelven programáticamente a partir de datos JSON
    generados por scripts de Python por proveedor.
*   Mantenga los pines reservados (por ejemplo, SWD en `PA13`/`PA14`) fuera del código generado
    a menos que se permita explícitamente.
*   Documente cualquier helper de plantilla en `README.md` y realice un seguimiento del trabajo pendiente en
    `docs/TODO-CREATOR-BSP.md`.

## Notas de Cobertura

La cobertura de LLVM está disponible usando `grcov`. Ejecute `make coverage` para construir las pruebas
con instrumentación y generar un informe HTML en `./coverage`. Al
recopilar la cobertura, asegúrese de que las siguientes variables de entorno estén configuradas (también están
presentes en `.cargo/config.toml`):

```
CARGO_INCREMENTAL=0
RUSTFLAGS="-Zinstrument-coverage"
LLVM_PROFILE_FILE="coverage-%p-%m.profraw"
```

Las futuras ejecuciones de Codex deben centrarse en la cobertura medible y usar estas variables
al generar pruebas.

Ejecute siempre `cargo fmt --all` y corrija los errores de formato antes de preparar una
solicitud de extracción. Verifique el formato con `cargo fmt --all -- --check`.

Las APIs públicas deben estar documentadas. El lint `#![deny(missing_docs)]` está habilitado en
todos los crates, por lo que la compilación fallará si cualquier elemento público carece de un
docstring significativo. Estos crates se publican en crates.io y requieren una documentación clara
para los usuarios.
Todos los archivos deben incluir un encabezado descriptivo que resuma su propósito.

Ejecute `./scripts/pre-commit.sh` y asegúrese de que se complete con éxito antes de abrir una solicitud de extracción. Este script aplica el formato, ejecuta clippy, construye con todas las características y verifica la generación de documentación usando nightly.

Use `scripts/check-links.sh` para validar los enlaces de Markdown antes de confirmar los cambios en la documentación.

## Scripts de Enlace de Ejemplo

Cada proyecto de ejemplo que proporciona un script de enlace `memory.x` debe incluir un
`build.rs` que:

*   copia el `memory.x` local en el directorio de salida de construcción de Cargo,
*   emite `cargo:rustc-link-search` para ese directorio, y
*   emite `cargo:rustc-link-arg=-Tmemory.x`.

Esto evita depender de un `.cargo/config.toml` global para la configuración del script de enlace.
