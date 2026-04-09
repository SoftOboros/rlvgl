```markdown
<!--
README.md - Top-level overview and navigation for rlvgl.
-->
<p align="centre">
  <img src="./rlvgl-logo.png" alt="rlvgl" />
</p>

<span style="font-size:26px"><b>rlvgl</b></span> es una reimplementación modular e idiomática en Rust de LVGL (Light and Versatile Graphics Library).

rlvgl conserva el paradigma de interfaz de usuario basado en widgets de LVGL, eliminando al mismo tiempo la gestión de memoria de estilo C insegura y el estado global. Esta biblioteca está estructurada para soportar entornos no_std, objetivos embebidos (por ejemplo, STM32H7) y backends de simulador para prototipos rápidos.

La versión C de LVGL se incluye como un submódulo de git para referencia y extracción de vectores de prueba, pero no se enlaza ni se compila en esta biblioteca.

## Objetivos
Paquete: `rlvgl`
- Preservar la arquitectura y el sistema de diseño de LVGL
- Reemplazar el manejo de memoria de C con la gestión de propiedad idiomática de Rust
- Soportar el vaciado de pantalla y la entrada embebida a través de embedded-hal
- Habilitar la jerarquía de widgets, estilos y eventos usando traits de Rust
- Usar crates de Rust existentes siempre que sea posible (por ejemplo, embedded-graphics, heapless, tinybmp)

## Características
- Soporte no_std + asignador
- Diseño de módulo basado en componentes (core, widgets, platform)
- Simulación posible a través de la flag de característica std-enabled
- Backends de pantalla y entrada conectables
- Soporte opcional de Lottie a través del crate `rlottie` para reproducción dinámica.
  Los objetivos embebidos deben pre-renderizar animaciones a APNG para un tamaño mínimo.

## Estructura del Proyecto
- [core](./core/README.md) – Trait base de widget, layout, despacho de eventos
- [widgets](./widgets/README.md) – Reimplementaciones nativas en Rust de widgets LVGL
- [platform](./platform/README.md) – Traits de pantalla/entrada y adaptadores HAL
- [ui](./ui/README.md) – Componentes de interfaz de usuario de nivel superior
- [examples](./examples/README.md) – Aplicaciones de ejemplo y demostraciones de placas
- [docs](./docs/README.md) – Documentación del proyecto y listas de tareas
- [lvgl](./lvgl/README.md) – Submódulo C (solo referencia)
- [chips/stm/bsps](./chips/stm/bsps/README.md) 🆕 – Stubs BSP de STM32 generados

## Bases de datos de chips de proveedor

Las definiciones de placas específicas del proveedor se encuentran en los crates [`chipdb/`](./chipdb/README.md). El
asistente `tools/gen_pins.py` agrega entradas de proveedor en blobs JSON,
mientras que `tools/build_vendor.sh` orquesta la generación y estampa
archivos de licencia. Al construir un crate de proveedor, configure `RLVGL_CHIP_SRC` al
directorio que contiene estos archivos JSON para que el script de construcción pueda
incrustarlos a través de `include_bytes!`.

## Generación de BSP de STM32CubeMX 🆕

`rlvgl-creator` 🆕 convierte proyectos `.ioc` de STM32 CubeMX en stubs de soporte de placas.
Los módulos generados se envían en
[`rlvgl-bsps-stm` 🆕](./chips/stm/bsps/README.md). El soporte de superposición `board`
anterior se mantiene pero está obsoleto.

## Generador BSP (`rlvgl-creator` 🆕)

`rlvgl-creator` 🆕 ofrece una pipeline de dos etapas para los paquetes de soporte de placas:

1.  **Importar** archivos de proyecto del proveedor (por ejemplo, `.ioc` de STM32CubeMX, `.mex` de NXP,
    YAML de RP2040). Cada adaptador extrae los datos del proveedor y emite un pequeño **IR** YAML
    neutro al proveedor que describe relojes, pines, DMA y periféricos.
2.  **Generar** código de inicialización de Rust renderizando plantillas MiniJinja
    contra el IR. Los usuarios pueden elegir entre paquetes de plantillas incorporados o proporcionar
    los suyos propios.

El adaptador STM32CubeMX también analiza los multiplicadores PLL y las selecciones de reloj del kernel periférico para que la configuración del reloj se pueda generar junto con la configuración de pines.

No se mantienen tablas por chip. Las reglas a nivel de clase se reutilizan en
instancias y proveedores. Las funciones alternativas se derivan de bases de datos de proveedores embebidos
generadas a partir de las fuentes XML oficiales; no se requiere JSON externo
en el momento de la generación. Los pines SWD reservados (`PA13`, `PA14`) son rechazados
a menos que se permitan explícitamente.

Flujo típico:

```bash
rlvgl-creator platform import --vendor st --input board.ioc --out board.yaml
rlvgl-creator platform gen --spec board.yaml --templates templates/stm32h7 \
  --out src/generated.rs
```

Los números de función alternativa se calculan a partir de la base de datos embebida en tiempo de ejecución
por `rlvgl-creator`, por lo que no es necesario generar o pasar un archivo JSON.

Para empaquetar bases de datos de chips de proveedor para pruebas o publicación, ejecute:

```bash
tools/build_vendor.sh
RLVGL_CHIP_SRC=chipdb/rlvgl-chips-stm/generated cargo build -p rlvgl-chips-stm
```

Para una visión general completa del flujo de trabajo de activos, consulte el [README de rlvgl-creator 🆕](./README-CREATOR.md).
Los detalles de los comandos se encuentran en [docs/CREATOR-CLI.md](./docs/CREATOR-CLI.md).

### Esquema IR

El paso de importación emite una especificación YAML concisa que describe la placa:

```yaml
mcu: STM32H747XIHx
package: LQFP176
power: { supply: smps, vos: scale1 }
clocks:
  sources: { hse_hz: 25000000 }
  pll:
    pll1: { m: 5, n: 400, p: 2, q: 4, r: 2 }
  kernels: { usart1: pclk2 }
pinctrl:
  - group: usart1-default
    signals:
      - { pin: PA9,  func: USART1_TX, af: 7, pull: none, speed: veryhigh }
      - { pin: PA10, func: USART1_RX, af: 7, pull: up,   speed: veryhigh }
peripherals:
  usart1:
    class: serial
    params: { baud: 115200, parity: none, stop_bits: 1 }
    pinctrl: [ usart1-default ]
reserved_pins: [ PA13, PA14 ]
```

Resumen de campos:

-   `mcu`, `package` – identificadores del proyecto del proveedor.
-   `power` – configuración de la fuente de alimentación; los valores se asignan directamente a las llamadas HAL.
-   `clocks` – frecuencias de entrada (`sources`), multiplicadores PLL (`pll`) y
    selecciones de kernel por periférico (`kernels`).
-   `pinctrl` – grupos de pines con sus funciones, funciones alternativas,
    pulls y velocidades.
-   `peripherals` – mapa de instancias de periféricos con clave por nombre (`usart1`),
    cada uno con una `class` (por ejemplo, `serial`) y `params` opcionales.
-   `dma`, `interrupts` – arrays opcionales que describen las solicitudes DMA y las
    prioridades IRQ.
-   `reserved_pins` – pines que no deben reconfigurarse (por ejemplo, SWD).

### Ayudantes de plantillas

Las plantillas MiniJinja pueden usar los siguientes filtros:

-   `pin_var` – convierte un pin como `PA9` en el nombre de variable `pa9`.
-   `periph_num` – extrae los dígitos finales de un nombre de periférico
    (`usart12` → `12`).
-   `af_alt` – renderiza un número de función alternativa para
    `into_alternate::<AF>()` (`7` → `<7>`).

Los usuarios pueden proporcionar plantillas personalizadas apuntando `--templates` a cualquier
directorio; los filtros anteriores siempre están disponibles.

Vea `docs/TODO-CREATOR-BSP.md` para el trabajo restante.

## Estado

Tal como está construido. Consulte [docs](./docs/README.md) para ver el progreso componente por componente y las tareas pendientes.

A partir de la versión 0.1.0, muchas características están implementadas y se ha logrado una cobertura de pruebas unitarias del 87%, pero no se han realizado pruebas funcionales ni pruebas en hardware real (bare-metal).

## Ejemplo Rápido

```rust
use rlvgl_core::widget::Rect;
use rlvgl_widgets::label::Label;

fn main() {
    let mut label = Label::new(
        "hello",
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 20,
        },
    );
    label.style.bg_color = rlvgl_core::widget::Color(0, 0, 255, 255);
    // Rendering would use a DisplayDriver implementation.
}
```

## Pruebas

Ejecute las pruebas basadas en el host con la cadena de herramientas predeterminada:

```bash
cargo test --workspace
```

Las pruebas de objetivo cruzado (por ejemplo, `thumbv7em-none-eabihf`) requieren un enlazador. Cargo
usa `arm-none-eabi-gcc` por defecto, pero puede evitar instalar GCC agregando
el componente `rust-lld` y configurando:

```bash
rustup component add rust-lld
```

```toml
[target.thumbv7em-none-eabihf]
linker = "rust-lld"
```

Consulte [docs/CROSS-TESTING.md](docs/CROSS-TESTING.md) para obtener consejos de solución de problemas.

## Cobertura

La instrumentación de cobertura LLVM se configura a través de `.cargo/config.toml` y el
objetivo `coverage` en el `Makefile`. Ejecute `make coverage` para ejecutar las pruebas
con instrumentación y generar un informe HTML en `./coverage/`.

## [rlvgl crate](https://crates.io/crates/rlvgl)
- El enlace anterior es para el crate principal, que agrupa a los demás e incluye el simulador.
- [rlvgl-core crate](https://crates.io/crates/rlvgl-core)
- [rlvgl-widgets crate](https://crates.io/crates/rlvgl-widgets)
- [rlvgl-platform crate](https://crates.io/crates/rlvgl-platform)

Ejecute el siguiente comando Cargo en el directorio de su proyecto:
```bash
cargo add rlvgl
```
O agregue la siguiente línea a su Cargo.toml:
```toml
rlvgl = "0.1.5"
```

## Comunidad
- [Código de Conducta](./CODE_OF_CONDUCT.md)
- [Notas del Contribuyente](./AGENTS.md)

## Docker Hub
La imagen de construcción utilizada por el flujo de trabajo de GitHub para este repositorio está disponible públicamente en [Docker Hub](https://hub.docker.com/r/iraa/rlvgl).
```bash
docker pull iraa/rlvgl:latest
```

Consulte el [Dockerfile](https://github.com/SoftOboros/rlvgl/blob/main/Dockerfile) para obtener detalles sobre el entorno de construcción.

Otros scripts de ayuda útiles se pueden encontrar en [`/scripts`](https://github.com/SoftOboros/rlvgl/blob/main/scripts).

## Licencia
rlvgl tiene licencia MIT. Consulte [LICENSE](./LICENSE) para obtener más detalles.
Los avisos de licencias de terceros se resumen en [NOTICES.md](./NOTICES.md).
```
