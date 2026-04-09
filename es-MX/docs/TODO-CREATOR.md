Okay, I will translate the Markdown content to es-MX, following all your requirements. I will go section by section.
The `replace` tool failed due to a relative path. I'll correct the file path to absolute `/Users/iraabbott/softoboros/docs/TODO-CREATOR.md` and retry the operation.
"File not found" error again. The absolute path should be correct. I'll use `list_directory` to double-check the file's existence and its parent directories.
Okay, `TODO-CREATOR.md` isn't directly in `/Users/iraabbott/softoboros/docs/`, despite the markdown context suggesting it. The initial directory listing confirms this. I'll use `glob` to locate its actual filesystem path within `/Users/iraabbott/softoboros/`.
I have identified three `TODO-CREATOR.md` files. The user requested translation *to* `es-MX`, implying an English source. As the provided content is English, I'll translate that to `es-MX` and output *only* the translated Markdown, without modifying any files. I will start by confirming the `en-CA` file's content matches the provided source.
```markdown
<!--
docs/TODO-CREATOR.md - rlvgl-creator — Épica y Tablas Seccionadas.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator — Épica y Tablas Seccionadas

_Un único archivo Markdown que estructura el trabajo como una **Épica** con tablas de historias de usuario seccionadas. Cada sección comienza con una breve descripción (historia de usuario) y una tabla de checklist._

---

## Resumen de la Épica
**Épica:** Construir **rlvgl-creator**, una herramienta de UI + CLI que importa, normaliza, previsualiza y vende activos para proyectos rlvgl, mientras que andamiaje de crates de activos de modo dual y minimiza la huella en objetivos `no_std + alloc`.

**Resultados:**
- Pipelines repetibles para secuencias de imágenes RGBA en bruto, fuentes y Lottie.
- Políticas estrictas de nombres/rutas con guía de autocorrección.
- Entrega dual (incrustar vs. vender) para paquetes de activos.
- Una interfaz de usuario de escritorio para previsualización, dimensionamiento y empaquetado.

---

## 0) Decisiones y Políticas Bloqueadas
_Historia de usuario: Como mantenedor, quiero salvaguardias para que los equipos puedan escalar los activos de forma segura sin desviaciones._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Imponer raíces de carpeta `icons/`, `fonts/`, `media/`; rechazar otras con guía de corrección. | creator core | Verificador de políticas + `--fix` renombrar.
| [x] | Generar nombres de constantes/características; prohibir ediciones manuales (SCREAMING_SNAKE; `ICON_`, `FONT_`, `MEDIA_`). | creator core | Mapa de nombres determinista; salida de diferencias.
| [x] | Creator es `std`; los objetivos son compatibles con `no_std + alloc`; pre-dimensionar/empaquetar activos. | N/A | Restricción de diseño en todas las características.
| [x] | Activos base almacenados como imágenes/secuencias RGBA en bruto; sin PNG/APNG en tiempo de ejecución. | interno | Reemplaza formatos dependientes de `std`.
| [x] | Admite tanto la reproducción directa de Lottie como la conversión de Lottie a APNG. | rlottie (FFI) o Conan CLI | Elección por activo registrada en el manifiesto.
| [x] | Compresión de paquete opcional utilizando RLE + tabla de tokens; la ruta del núcleo decodifica con un decodificador diminuto y con puerta. | interno | compatible con `no_std`; prefiere la compresión en tiempo de compilación en la ruta del proveedor.

---

## 1) Interfaz CLI y UX
_Historia de usuario: Como desarrollador, puedo gestionar activos mediante comandos claros con validación útil y ejemplos._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | `init` — arrancar carpetas y `manifest.yml` predeterminado. | clap, anyhow | Idempotente; imprime los siguientes pasos.
| [x] | `scan <path>` — descubrir activos nuevos/cambiados y actualizar el manifiesto. | blake3, walkdir | Basado en hash; respeta la política de raíces.
| [x] | `convert` — normalizar a secuencias RGBA en bruto; empaquetar fuentes; escribir metadatos. | image, fontdue/ab_glyph | Salidas deterministas.
| [x] | `vendor` — copiar a `$OUT_DIR`/repo y generar `rlvgl_assets.rs`. | std fs, tera | Admite preajustes por objetivo.
| [x] | `scaffold assets-crate` — generar crate de modo dual. | tera | Funciones de incrustación y venta.
| [x] | `preview` — miniaturas/hojas de sprites. | image | Almacena en `assets/thumbs/`.
| [x] | `add-target` — registrar crate local + `vendor_dir` y preajustes. | serde_yaml | Actualiza el manifiesto.
| [x] | `sync` — regenerar características de Cargo, constantes, índice del manifiesto. | tera | El modo de simulación imprime la diferencia.
| [x] | `apng` — construir APNG a partir de grupos de fotogramas en bruto; establecer temporización/bucles. | apng | Exportación del primer fotograma PNG.
| [x] | `lottie import` — Lottie→fotogramas/APNG; exportar mapa de temporización. | rlottie/CLI | Registra la ruta elegida.
| [x] | `fonts pack` — tamaños, conjuntos de glifos, empaquetado/métricas. | fontdue/ab_glyph | Subconjunto opcional.
| [x] | `check` — validación estricta de políticas; `--fix` auto-normalizar. | creator core | Salida no cero en caso de infracciones.
| [ ] | `ui` — iniciar interfaz de usuario de escritorio. | Tauri o eframe/wgpu | Comparte librerías centrales.
| [x] | Proporcionar banderas globales y ayuda enriquecida con ejemplos. | clap | Códigos de salida estandarizados.
| [x] | Dividir la implementación de CLI en módulos. | interno | Mantiene los binarios mantenibles.

---

## 2) Manifiesto y Convenciones
_Historia de usuario: Como mantenedor, quiero un manifiesto propiedad de la máquina que codifique la política y los objetivos._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Definir `manifest.yml` v1 (`packages`, `groups`, `features`, `expose`, `targets`). | serde_yaml, schemars | Emite esquema JSON para herramientas de edición.
| [x] | Imponer política de rutas: rutas públicas bajo `icons/`, `fonts/` o `media/`. | creator core | Errores accionables + `--fix`.
| [x] | Generar nombres de características a partir de grupos; emitir agregados `*_all`. | creator core | Orden estable.
| [x] | Generar nombres de constantes a partir de entradas de manifiesto; rechazar renombres manuales. | creator core | Las diferencias imprimen el mapeo antiguo→nuevo.
| [x] | Metadatos de licencia por activo/grupo con lista de permitidos/denegados. | tabla SPDX | Bloquear proveedor si falta.
| [x] | Configuración de `naming` (mapa de prefijos + política de casos) para documentos; el generador es la fuente de la verdad. | N/A | Mantiene la política explícita.
| [x] | Preajustes por objetivo (tamaño de pantalla, profundidad, almacenamiento) para el dimensionamiento automático. | archivo de preajustes | Conectado a `vendor`.

---

## 3) Andamiaje de Crate de Activos (Modo Dual)
_Historia de usuario: Como usuario, puedo consumir activos incrustando bytes o vendiendo archivos sin dependencias de tiempo de ejecución._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Generar `Cargo.toml` con características `embed`, `vendor` y de grupo. | tera | Sin características predeterminadas.
| [x] | Generar `src/lib.rs` — incrustar: constantes `include_bytes!`. | tera | Una constante por activo expuesto.
| [x] | Generar `src/lib.rs` — proveedor: `vendor_api::{copy_all, generate_rust_module}`. | std fs, tera | Rutas seguras para `$OUT_DIR`.
| [x] | Auto-prueba `build.rs` opcional para el crate. | std | Prueba de humo en CI.
| [x] | Generar README con uso de incrustación vs. proveedor. | tera | Fragmentos de copiar y pegar.
| [x] | Pruebas de instantáneas para archivos generados. | insta | Regresiones de guardia.
| [x] | `cargo publish --dry-run` pasa. | cargo | Puerta de CI.

---

## 4) Pipelines de Conversión
_Historia de usuario: Como diseñador, puedo soltar formatos comunes y obtener salidas normalizadas y de carga rápida._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Formato de secuencia RGBA en bruto con encabezado de fotogramas máximos; tamaño/posición por fotograma; las imágenes individuales eliminan los encabezados de fotogramas. | interno | Reemplaza la base PNG/APNG.
| [x] | Codificar archivos `.raw` a partir de entradas comunes. | creator core | Normaliza los activos ráster.
| [x] | Ingestar archivos `.raw` en la pipeline. | creator core | Analizar encabezado y fotogramas.
| [x] | SVG→imágenes en bruto dimensionadas (lista de DPI; umbrales monocromáticos/e-ink). | resvg/usvg (opt.) | Recurso a externo si es necesario.
| [x] | Constructor de APNG a partir de fotogramas en bruto con retardo por fotograma y recuento de bucles; primer fotograma PNG. | apng | Comprobaciones de orden de fotogramas.
| [x] | Lottie a través de FFI (`lottie-ffi`) usando `rlottie`. | rlottie, Conan | Puerta de características; notas de plataforma.
| [x] | Lottie a través de CLI externo (`lottie-cli`) a fotogramas/APNG. | receta de Conan | Registra la ruta en el manifiesto.
| [x] | Fuentes: TTF/OTF→paquetes de mapas de bits (`.bin`) + métricas (`.json`); subconjunto opcional. | fontdue/ab_glyph | Conjunto de glifos por objetivo.
| [x] | Compresión simple RLE + tabla de tokens para archivos en bruto. | interno | Decodificador diminuto para objetivos `no_std`.

---

## 5) Aplicación de UI (Interfaz de Usuario del Creador)
_Historia de usuario: Como desarrollador/diseñador, puedo previsualizar, hacer zoom/pan, agrupar y exportar activos visualmente._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Elegir pila e iniciar proyecto de UI. | eframe/egui | Ventana inicial y carga de manifiesto.
| [x] | Panel del Navegador de Activos (árbol, filtros, búsqueda, insignias de licencia). | kit de UI | Refleja los grupos del manifiesto.
| [x] | Visor de Lienzo (zoom/pan, cuadrícula de píxeles, fondo de tablero de ajedrez). | wgpu/pixels | Limpiador APNG/Lottie pendiente.
| [x] | Inspector: Meta (tamaño/DPI/hash/licencia/etiquetas/grupos). | serde | La edición en vivo escribe el manifiesto.
| [x] | Inspector: Exportar (tamaños, espacio de color, alfa pre-multiplicado, compresión). | creator core | Se aplica por activo.
| [x] | Inspector: Animación (temporización/bucles; opciones de Lottie→APNG). | apng/rlottie | UI del limpiador.
| [x] | Inspector: Fuentes (conjunto de glifos, tamaños, sugerencias, empaquetado). | fontdue | Previsualizar pangramas.
| [x] | Arrastrar y soltar a `assets/raw/` con `scan` inmediato. | notify | Muestra brindis.
| [x] | Preajustes de tamaño a pantalla (p. ej., `stm32h7‑480x272`) con vista previa en vivo. | preajustes | Renderiza cuadros delimitadores.
| [x] | Acciones: "Crear APNG a partir de la selección", "Añadir a grupo", "Mostrar en manifiesto". | kit de UI | Soporte de selección múltiple.
| [x] | Pipeline de miniaturas + recarga en caliente. | image, notify | Invalidación de caché mediante hash.
| [x] | Vista previa/editor de diseño para prototipado rápido de UI. | más tarde | Lienzo de diseño básico de arrastrar y soltar.

---

## 6) Integración de Vendedor y Incrustación
_Historia de usuario: Como autor de la aplicación, puedo elegir incrustar o vender y obtener bytes idénticos._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Ejemplos de incrustación (`default-features=false`, características por grupo; uso de constantes). | ejemplos | CI los construye.
| [ ] | Ejemplos de proveedor (`build.rs` del consumidor + `include!(.../rlvgl_assets.rs)`). | ejemplos | Seguro para `$OUT_DIR`.
| [x] | API `get(path)` opcional en modo incrustado (ruta→bytes). | phf/lite map | Índice generado.
| [x] | Prueba de igualdad de bytes: incrustar vs. proveedor para las mismas IDs de activos. | pruebas | Aserción de CI.

---

## 7) Caché y Construcciones Incrementales
_Historia de usuario: Como usuario, quiero ejecuciones rápidas con salidas deterministas._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Caché de hash de contenido en `assets/.cache` (hash→salidas/marcas de tiempo/tamaños). | blake3, serde | Almacén JSON/CBOR.
| [x] | Invalidación `--force` y reconstrucción inteligente por hash/mtime. | creator core | Mensajería clara.
| [x] | Paralelizar conversiones con ordenación estable. | rayon (opt.) | Condiciones de carrera de guardia.
| [x] | Emitir sugerencias `cargo:rerun-if-changed` para pasos de proveedor/construcción. | API build.rs | Buen DX para los consumidores.

---

## 8) Validación, Linting y CI
_Historia de usuario: Como mantenedor, puedo confiar en que cada PR aplicará la política y se mantendrá en verde._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | `creator check` cubre rutas, nombres, licencias, duplicados, umbrales de tamaño. | creator core | Salida no cero.
| [x] | Plantilla de gancho de pre-commit (escanear/convertir/verificar). | git hooks | Opcional pero recomendado.
| [x] | El trabajo de CI se ejecuta de extremo a extremo: `scan → convert → sync → scaffold → vendor`. | GH Actions | Almacena en caché toolchains.
| [x] | Pruebas "Golden" para la temporización de APNG y muestras de fuentes. | apng, fontdue | Accesorios deterministas.
| [x] | Pruebas de instantáneas para `Cargo.toml`, `lib.rs`, `rlvgl_assets.rs` generados. | insta | Almacenado en el repositorio.

---

## 9) Criterios de Aceptación (MVP)
_Historia de usuario: Como interesado, puedo verificar el valor rápidamente con una sección vertical funcional._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Crate de activos de modo dual se compila desde el andamiaje. | cargo | prueba de humo.
| [ ] | `scan + convert + sync` coinciden con el manifiesto; sin archivos extraviados. | creator core | Verificación de CI.
| [x] | El proveedor y la incrustación producen bytes idénticos para las mismas IDs de activos. | pruebas | Comparación de bytes.
| [ ] | APNG de fotogramas simples se reproduce con la temporización correcta en un visor de referencia. | apng | Visor en CI (sin cabeza).
| [ ] | `cargo publish --dry-run` para el crate generado tiene éxito. | cargo | Reglas de versionado.
| [ ] | Las entradas no conformes obtienen errores accionables y `--fix` los resuelve. | creator core | Salida fácil de usar.

---

## 10) Hoja de Ruta / Fases
_Historia de usuario: Como planificador, puedo organizar la entrega para obtener valor de forma temprana y frecuente._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Fase 1 – MVP: escanear/convertir/vender; andamiaje de crate; verificación estricta. | piezas centrales | Lanzamiento base.
| [x] | Fase 2 – Fuentes: subconjunto/empaquetado/métricas; grupos de características por tamaño/familia. | fontdue | Mejora el rendimiento de carga.
| [x] | Fase 3 – Lottie: importar + APNG; hojas de sprites + meta de temporización. | rlottie/apng | Soporte de animación más amplio.
| [x] | Fase 4 – Vista previa: miniaturas + visor CLI/GUI; perfilado de tamaño/ruta crítica. | UI + image | Velocidad del desarrollador.
| [x] | Fase 5 – GUI: UI completa con vista previa/editor de diseño y preajustes. | pila de UI | Velocidad del diseñador.
| [ ] | Fase 6 – Avanzado: pipelines wasm; catálogos remotos; empaquetado CDN. | wasm-bindgen | Extensión.

---

## 11) Adicionales y Deseables
_Historia de usuario: Como usuario avanzado, puedo optimizar aún más las pipelines y el empaquetado._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Constructor de hoja de sprites/atlas (+ atlas JSON/RON). | image, serde | Opción para partículas/UI.
| [ ] | Preajustes y asistentes por objetivo (restricciones de pantalla/bpp/almacenamiento). | preajustes | UX del asistente.
| [ ] | Puerta de licencia en el proveedor (bloquear activos incompatibles). | SPDX | Seguridad legal.
| [ ] | Telemetría local: bytes guardados, estimaciones de tiempo de carga y RAM/flash. | módulo de estadísticas | Opción de participación.
| [ ] | Puntos de extensión para convertidores/optimizaciones personalizados. | APIs de rasgos | Cargar desde TOML.

---

## 12) Entregables y Documentos
_Historia de usuario: Como recién llegado, puedo ser productivo con ejemplos y guías claras._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Paquete de activos de ejemplo (iconos/fuentes/medios) con manifiesto. | datos del repositorio | Utilizado en pruebas.
| [ ] | Dos ejemplos de consumidor: patrones de **incrustación** y **proveedor**. | ejemplos | CI construye y ejecuta.
| [x] | Guía de usuario (README) con flujo de trabajo de extremo a extremo. | mdbook/README | Capturas de pantalla/gifs.
| [x] | Documentación del desarrollador para plantillas (Tera) y ganchos de pipeline. | rustdoc | API + directorio de plantillas.
```
