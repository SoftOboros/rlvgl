```markdown
<!--
docs/TODO.md - Pendientes del Proyecto.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Pendientes del Proyecto

Este documento registra las líneas de trabajo y tareas de alto nivel para el desarrollo de rlvgl.

## 0 Inicialización de Repositorio y Contenedor
 - [x] Crear esqueleto de monorepo rlvgl (`core/`, `widgets/`, `platform/`, `lvgl/` submodule)
- [x] Finalizar ajustes de Dockerfile (LLVM/clang, rustup, arm-gcc, bindgen, SDL)
- [x] Añadir `.cargo/config.toml` con perfil incrustado y triple de destino
- [x] Stub de GitHub Actions / GitLab CI (construcción, pruebas unitarias, informe de tamaño)

## 1 Esqueleto del entorno de ejecución central
- [x] Trait de Widget (límites, ganchos de ciclo de vida, firmas de dibujo/evento)
- [x] Árbol WidgetNode (Rc<RefCell<_>>, o índice de losa)
- [x] Enumeración de Eventos + despachador (orden de burbujeo/captura)
- [x] Estructura de Estilo + patrón constructor
- [x] Trait de Renderizador Mínimo (agnóstico al objetivo)

## 2 Herramientas de construcción e Integración Continua (CI)
- [x] Añadir configuraciones de `profile.release` (lto=true, opt-level=z, etc.)
- [x] Script de verificación de tamaño (`arm-none-eabi-size`)
- [x] Comprobación de Clippy + Rust-fmt
- [x] Plantilla de gancho pre-commit
- [x] Script de inicialización de entorno de CI + integración de flujo de trabajo
## 3 Capa HAL de Pantalla y Entrada

- [x] Definir trait `DisplayDriver` (`flush(Rect, &[Color])`)
- [x] Definir trait `InputDevice` (`poll() -> Option<InputEvent>`)
- [x] Proporcionar controlador stub ficticio para pruebas sin interfaz gráfica
- [x] Controlador de ejemplo basado en SPI (`st7789`) usando `embedded-hal`

## 4 Traducciones de widgets de Nivel 1
- [x] Etiqueta (solo texto)
- [x] Botón (extiende propiedades de Etiqueta)
- [x] Contenedor (diseño tipo flex)
- [x] Pruebas unitarias + capturas de pantalla de referencia mediante controlador ficticio

## 5 Backend de simulación
- [x] Añadir bandera de característica de simulador (`std`, pixels)
- [x] Mapear `DisplayDriver` a ventana de escritorio
- [x] Conectar teclado/ratón al trait `InputDevice`
- [ ] Paso de CI: ejecutar ejemplo, volcar PNG para diff

## 6 Traducciones de widgets de Nivel 2
- [x] Casilla de verificación
- [x] Deslizador
- [x] Arco / Barra de progreso
- [x] Lista
- [x] Imagen (backend de gráficos embebidos)

## 7 Temas y Animaciones
 - [x] Trait de Tema Global (esquema de color, cascada de estilos)
 - [x] Gestor de Animación (gancho `tick()` → interpolación de estilo/posición)
 - [x] Portar fundido/deslizamiento básico de LVGL como prueba

## 8 Documentación y Ejemplos
- [x] Generar documentación automática para cada API pública con `#![doc = include_str!(…)]`
- [ ] Generar sitio mdBook o Docusaurus
- [ ] Galería de ejemplos: incrustar GIFs generados por el simulador

## 9 Pruebas avanzadas y arnés de regresión
- [ ] Extraer demo C de LVGL → renderizar a mapa de bits (sim)
- [ ] Renderizar la misma UI en rlvgl, diff de imagen en CI
- [ ] Fuzzing de eventos (pulsaciones rápidas, arrastres) para detectar panics de préstamo/tiempo de vida

## 10 Ejemplos de simulador
- [x] Actualizar samples/sim (rlvds-sim) para ejecutar rlvds en una ventana usando std y la característica "simulator"
- [x] Demostrar características principales / de widgets en examples/sim (usar activos de marcador de posición) usando jerarquía que coincida con el paquete superior
- [x] Demostrar características de plugin (usar activos de marcador de posición) usando jerarquía que coincida con el paquete superior; añadir un botón `plugins` a la demo para lanzar elementos opcionales y configurar todas las opciones de construcción
```
