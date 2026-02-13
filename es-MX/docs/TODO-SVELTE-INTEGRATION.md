```markdown
<!--
docs/TODO-SVELTE-INTEGRATION.md - rlvgl-creator Svelte alignment and token-driven UI pipeline TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator — Tareas Pendientes de Integración con Svelte

_Un único archivo markdown que estructura el trabajo como una **Épica** con tablas de historias de usuario seccionadas. Cada sección comienza con una breve descripción (historia de usuario) y una tabla de verificación._

---

## Resumen de la Épica
**Épica:** Alinear Svelte (tokens de diseño, creación de componentes y estado de runes) con `rlvgl` extendiendo `rlvgl-creator` para generar archivos para objetivos web y embebidos a partir de fuentes de UI compartidas.

**Resultados:**
- La fuente de tokens compartida produce salidas CSS/Tailwind para la web y temas `rlvgl-ui`.
- La creación de componentes Svelte se mapea a árboles de widgets `rlvgl` (subconjunto con restricciones claras).
- Las runes de Svelte 5 se mapean a enlaces de estado embebidos y actualizaciones derivadas.
- Una futura construcción dual (simulador web + embebido) es habilitada por IR compartido y ganchos de generador.

---

## 0) Decisiones y Restricciones Bloqueadas
_Historia de usuario: Como mantenedor, quiero límites claros para que la integración siga siendo solo de generación de archivos y esté alineada con los crates existentes._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [x] | Creator sigue siendo solo generación de archivos (sin ejecución en tiempo de ejecución). | policy | Genera solo Rust/JS/CSS/config. |
| [x] | No hay nuevos crates para esta fase de alineación. | workspace | Añadir módulos bajo `src/bin/creator/`. |
| [x] | La dirección principal es **Sistema de Diseño Compartido** (B), con prototipos (A) más tarde. | product | Tokens primero, creación de UI segundo. |
| [x] | Comenzar con la opción 5 (Svelte → WASM → rlvgl renderer) diseño, pero solo generar archivos y ganchos. | architecture | Trabajo en tiempo de ejecución aplazado. |
| [x] | Proporcionar ganchos para la construcción dual (Opción 4) temprano; entregarlo más tarde. | architecture | IR y manifiestos deben soportar ambos. |

---

## 1) Superficie CLI: Nuevo Comando `svelte`
_Historia de usuario: Como desarrollador, puedo ejecutar comandos explícitos del creador para generar salidas de tokens, objetivos de componentes y código pegamento a partir de fuentes de Svelte._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Añadir el comando de nivel superior `rlvgl-creator svelte` con subcomandos. | clap | Nueva familia de comandos. |
| [ ] | `svelte tokens` — lee YAML de tokens y emite salidas web + rlvgl. | serde_yaml | Emite CSS/Tailwind + Rust. |
| [ ] | `svelte compile` — compila `.svelte` a IR y emite Rust de widgets rlvgl. | Svelte parser/CLI | Salida solo de archivo. |
| [ ] | `svelte wasm` — emite pegamento del renderizador y configuraciones de construcción para Svelte→WASM→rlvgl. | templates | Genera solo shims. |
| [ ] | `svelte schema` — emite esquema JSON para tokens e IR de UI. | schemars | Soporte de editor. |
| [ ] | `svelte check` — valida tokens + restricciones del subconjunto de Svelte. | creator core | Salida distinta de cero en violaciones. |

---

## 2) Capa de Tokens Compartida
_Historia de usuario: Como diseñador, defino los tokens una vez y los consumo en objetivos web y embebidos de manera consistente._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Definir esquema `shared-tokens.yaml` (colores, espaciado, radios, tipografía, movimiento). | schemars | Los colores permiten hex/rgb/rgba; el espaciado/radios son px; el movimiento son ms + tokens de easing. |
| [ ] | Añadir capas de tokens base + semánticos con modos opcionales (claro/oscuro/alto contraste). | creator core | La V1 usa un solo modo; si existen múltiples, usa el predeterminado/primero o requiere selección explícita. |
| [ ] | Permitir alias de tokens con detección de ciclos. | creator core | Error en referencias circulares. |
| [ ] | Normalizar nombres de tokens en identificadores determinísticos. | creator core | Política de mayúsculas/minúsculas + mapa de prefijos. |
| [ ] | Definir sintaxis de referencia de tokens para fuentes de UI y código generado. | docs | Usar `token("colors.primary")` en fuentes de Svelte. |
| [ ] | Emitir `tokens.json` normalizado para consumidores de IR. | serde_json | Mapa de tokens canónico para compiladores. |
| [ ] | Generar salida de propiedades personalizadas CSS (`tokens.css`). | templates | Salida para Svelte/web. |
| [ ] | Generar fragmento de configuración de Tailwind (`tailwind.tokens.cjs`). | templates | Integración opcional. |
| [ ] | Generar módulo Rust de tema `rlvgl-ui` (`theme.rs`). | templates | Estructuras `Theme`/`Palette`. |
| [ ] | Añadir sección de manifiesto para procedencia y versionado de tokens. | manifest | Rastrea la fuente + hash. |

---

## 3) IR de Componentes Svelte (Subconjunto)
_Historia de usuario: Como desarrollador, puedo crear un componente Svelte restringido que se mapea limpiamente a la salida de UI embebida._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Definir reglas de subconjunto de Svelte (sin APIs DOM, sin `{@html}`, slots limitados). | docs | Permitir solo el slot predeterminado; validar en `svelte check`. |
| [ ] | Definir bloques/directivas permitidos (`{#if}`, `{#each}` con clave, `on:` eventos, `bind:`). | docs | No `{#await}`, no `use:`, no `transition:` todavía. |
| [ ] | Definir etiquetas/componentes permitidos (etiquetas solo de rlvgl, sin HTML puro). | docs | Comenzar con Button/Text/Image/Stack/Row/Column. |
| [ ] | Implementar el análisis de `.svelte` a un IR del creador (componentes, props, hijos, estilos). | parser/CLI | Preferir parser Svelte externo si es necesario. |
| [ ] | Definir campos IR para enlaces dinámicos (refs de tokens vs refs de estado). | IR | Distinguir valores estáticos vs derivados. |
| [ ] | Mapear enlaces `style:` de Svelte a referencias de tokens y estilos rlvgl. | creator core | Tokens como fuente de la verdad. |
| [ ] | Normalizar eventos (`on:click`, etc.) a callbacks rlvgl. | IR | Definir reglas de firma de manejadores. |
| [ ] | Serializar IR a JSON para futuras herramientas. | serde_json | Habilita la construcción dual más tarde. |

---

## 4) Objetivo Svelte → rlvgl (Dirección B)
_Historia de usuario: Como desarrollador, puedo compilar un componente Svelte en un árbol de widgets rlvgl con estilos y eventos._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Construir tabla de mapeo de widgets (etiqueta Svelte → widget rlvgl). | docs | Comenzar con Button, Text, Image, Stack, Row, Column. |
| [ ] | Definir mapeo de props de diseño (tamaño, relleno, espacio, alinear, justificar). | rlvgl-ui | Asegurar valores predeterminados determinísticos. |
| [ ] | Generar código Rust builder para árboles de widgets. | templates | Solo salida. |
| [ ] | Soportar mapeo de estilos (fondo, relleno, radio, fuente, color). | rlvgl-ui | Enlazar a la salida de tokens. |
| [ ] | Emitir módulos de componentes con APIs públicas estables. | templates | Coincidir con las convenciones `rlvgl`. |
| [ ] | Añadir pruebas que compilen un archivo Svelte de ejemplo en salida Rust. | tests | Instantáneas doradas. |

---

## 5) Runas de Svelte 5 → Modelo de Estado rlvgl
_Historia de usuario: Como desarrollador, puedo mapear `$state`, `$derived`, `$effect` de Svelte a primitivas de estado embebidas._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Definir un IR de estado mínimo (`State`, `Derived`, `Effect`). | creator core | Salida solo de archivo. |
| [ ] | Mapear `$state` a `State<T>` y `$derived` a callbacks computados. | rlvgl-ui | Añadir o reutilizar ayudantes de estado. |
| [ ] | Definir patrones de script permitidos (sin async, sin DOM, sin stores externos). | docs | Solo runes + funciones locales. |
| [ ] | Definir reglas de enlace para `bind:` (ej. `bind:value`, `bind:checked`). | docs | Mapear a setters/getters de estado. |
| [ ] | Definir restricciones de programación de efectos para objetivos embebidos. | docs | Sin efectos secundarios asíncronos; ejecutar en cambio de estado. |
| [ ] | Generar módulos Rust para cableado de estado y callbacks. | templates | Enlazar a eventos de widgets. |
| [ ] | Añadir errores de validación para patrones de reactividad de Svelte no soportados. | creator core | Mensajes útiles. |

---

## 6) Ganchos de la Opción 5: Svelte → WASM → Renderizador rlvgl
_Historia de usuario: Como desarrollador, puedo generar el código pegamento necesario para conectar el tiempo de ejecución de Svelte a un renderizador rlvgl, sin que el creador ejecute nada._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Definir una superficie de API de renderizador para enlaces de tiempo de ejecución de Svelte. | docs | Crear/actualizar/eliminar nodos, establecer props, establecer estilos, despachar eventos. |
| [ ] | Generar shims `wasm-bindgen` de Rust para puntos de entrada del renderizador. | templates | Solo salida de archivo. |
| [ ] | Emitir pegamento JS que reenvía operaciones DOM a enlaces de renderizador. | templates | Adaptador de tiempo de ejecución de Svelte. |
| [ ] | Generar fragmentos de configuración de construcción (`Cargo.toml`, `package.json`) solo como plantillas. | templates | Sin ejecución. |
| [ ] | Documentar las características de Svelte soportadas en modo WASM. | docs | Subconjunto mínimo viable. |

---

## 7) Construcción Dual (Opción 4) — Planeada para Más Tarde
_Historia de usuario: Como desarrollador, puedo construir una vista previa web y un objetivo embebido desde la misma fuente de UI._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Definir un IR compartido que pueda emitir salidas web y rlvgl. | IR | Reutilizar de la sección 3. |
| [ ] | Emitir salida de vista previa web (Svelte + tokens) en un paquete `preview/`. | templates | Solo archivos estáticos. |
| [ ] | Añadir comando `svelte preview` para generar paquete de vista previa. | creator CLI | Sin servidor de desarrollo. |
| [ ] | Añadir secciones de manifiesto para paquetes de vista previa y rutas de salida. | manifest | Rastrea hashes para reconstrucciones. |

---

## 8) Puntos de Integración en el Creador
_Historia de usuario: Como mantenedor, puedo integrar la alineación de Svelte sin nuevos crates y mantener el código modular._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Añadir módulo `src/bin/creator/svelte.rs` para entrada y orquestación CLI. | creator core | Refleja otros comandos. |
| [ ] | Añadir submódulos `src/bin/creator/svelte/`: tokens, ir, compile, wasm, check. | internal | Mantener módulos pequeños. |
| [ ] | Extender manifiesto con configuración `svelte` (ruta de tokens, raíces de UI, salidas). | manifest | Rastrea hashes para reconstrucciones. |
| [ ] | Conectar comandos Svelte en menús de UI más tarde (paridad post-CLI). | creator_ui | Seguimiento opcional. |

---

## 9) Validación, Pruebas y Documentación
_Historia de usuario: Como mantenedor, puedo confiar en las salidas generadas y entender el subconjunto claramente._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Añadir instantáneas doradas para salidas de tokens (CSS/Tailwind/Rust). | insta | Formato determinístico. |
| [ ] | Añadir accesorios `.svelte` de muestra para pruebas de compilación. | tests | Mantener subconjunto mínimo. |
| [ ] | Documentar el subconjunto de Svelte y la tabla de mapeo. | docs | Restricciones + ejemplos. |
| [ ] | Añadir entradas de referencia CLI del creador para subcomandos `svelte`. | docs | Actualizar `docs/CREATOR-CLI.md`. |

---

## 10) Hoja de Ruta / Fases
_Historia de usuario: Como planificador, puedo escalonar la entrega para obtener valor temprano y de forma segura._

| Completo | Descripción | Dependencias | Notas |
|---|---|---|---|
| [ ] | Fase 1 – Pipeline de tokens + esquema + `svelte tokens`. | tokens | Valor B inmediato. |
| [ ] | Fase 2 – Subconjunto de Svelte + IR + `svelte compile` a rlvgl. | parser | Dirección B. |
| [ ] | Fase 3 – Mapeo de runes y generación de estado. | rlvgl-ui | Dirección B. |
| [ ] | Fase 4 – Ganchos de la Opción 5 (pegamento del renderizador WASM). | templates | Salida solo de archivo. |
| [ ] | Fase 5 – Paquete de vista previa de construcción dual (Opción 4). | preview | Dirección A. |
```
