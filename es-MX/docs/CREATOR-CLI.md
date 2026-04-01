<!--
CREATOR-CLI.md - Command-line reference and workflows for rlvgl-creator.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# CLI de rlvgl-creator

## Resumen
`rlvgl-creator` es una utilidad de línea de comandos que prepara activos y paquetes de soporte de placa (BSPs) para proyectos `rlvgl`. Convierte archivos brutos a formatos adecuados para objetivos embebidos, gestiona manifiestos que rastrean metadatos de activos, y puede traducir archivos de configuración de proveedores a código Rust mediante renderizado de plantillas.

Un flujo de trabajo típico inicializa un paquete de activos, importa recursos, los convierte para un objetivo y estructura un crate que expone los activos en tiempo de compilación. Los flujos de trabajo de hardware analizan archivos de proveedores como las descripciones `.ioc` de STM32CubeMX y renderizan código BSP usando plantillas MiniJinja.

## Flujo de trabajo de inicio rápido
```bash
rlvgl-creator init
rlvgl-creator add-target host vendor
rlvgl-creator scan assets/
rlvgl-creator convert assets/
rlvgl-creator preview assets/
rlvgl-creator scaffold assets-pack
```
Esta secuencia crea un nuevo paquete de activos, registra un objetivo `host` cuyos activos convertidos se escriben bajo `vendor/`, escanea los directorios de activos brutos, convierte los activos a formas normalizadas, genera miniaturas para una revisión rápida, y finalmente estructura un crate de modo dual llamado `assets-pack` para incrustar o vender recursos.

## Referencia de comandos
### init
Inicializa directorios de activos (`icons/`, `fonts/`, `media/`) y escribe un `manifest.yml` vacío.

```
rlvgl-creator init
```

### scan
Escanea un árbol de directorios en busca de activos, calcula hashes y actualiza el manifiesto.

```
rlvgl-creator scan <path>
```
* `path` – directorio raíz que contiene los activos brutos.

### check
Valida las entradas del manifiesto contra los archivos de activos.

```
rlvgl-creator check <path> [--fix]
```
* `path` – directorio raíz que contiene los activos.
* `--fix` – escribe correcciones en el manifiesto cuando se encuentran discrepancias.

### vendor
Copia los activos procesados en un directorio de salida y emite un módulo de ayuda `rlvgl_assets.rs`.

```
rlvgl-creator vendor <path> <out> [--allow LICENSE] [--deny LICENSE]
```
* `path` – directorio raíz que contiene los activos.
* `out` – directorio donde se escriben los activos vendidos.
* `--allow` – lista blanca de licencias permitidas.
* `--deny` – lista negra de licencias no permitidas.

### convert
Normaliza activos (fuentes, imágenes, medios) y actualiza los metadatos del manifiesto.

```
rlvgl-creator convert <path> [--force]
```
* `path` – directorio raíz que contiene los activos.
* `--force` – reconstruye todos los activos incluso si existen salidas cacheadas.

### preview
Genera miniaturas bajo `thumbs/` para una inspección visual rápida.

```
rlvgl-creator preview <path>
```
* `path` – directorio raíz que contiene los activos.

### add-target
Registra un objetivo con nombre y el directorio donde se colocarán sus activos vendidos.

```
rlvgl-creator add-target <name> <vendor_dir>
```
* `name` – identificador utilizado en `manifest.yml`.
* `vendor_dir` – ruta donde se venden los activos convertidos.

### sync
Regenera las listas de características de Cargo y un índice de activos a partir del manifiesto.

```
rlvgl-creator sync <out> [--dry-run]
```
* `out` – directorio para escribir los archivos generados.
* `--dry-run` – imprime los cambios sin escribir en el disco.

### scaffold
Crea un crate de activos de modo dual que puede incrustar recursos o venderlos en tiempo de compilación.

```
rlvgl-creator scaffold <path>
```
* `path` – directorio de destino para el crate generado.

### apng
Crea un PNG animado a partir de una secuencia de cuadros.

```
rlvgl-creator apng <frames> <out> [--delay MS] [--loops N]
```
* `frames` – directorio que contiene cuadros PNG secuenciales.
* `out` – archivo APNG de salida.
* `--delay` – retardo de cuadro en milisegundos (predeterminado 100).
* `--loops` – número de bucles (`0` para infinito).

### schema
Imprime el esquema JSON para `manifest.yml` en la salida estándar.

```
rlvgl-creator schema
```

### fonts pack
Rasteriza fuentes TTF/OTF en datos de mapa de bits y archivos de métricas.

```
rlvgl-creator fonts pack <path> [--size PX] [--chars STRING]
```
* `path` – directorio que contiene los archivos de fuentes.
* `--size` – tamaño en puntos para la rasterización (predeterminado `32`).
* `--chars` – cadena de caracteres a incluir en el paquete.

### lottie import
Importa una animación Lottie JSON a cuadros PNG y, opcionalmente, a un APNG.

```
rlvgl-creator lottie import <json> <out> [--apng FILE]
```
* `json` – ruta al archivo Lottie JSON.
* `out` – directorio donde se escriben los cuadros.
* `--apng` – archivo APNG opcional para generar.

### lottie cli
Utiliza una CLI externa para convertir una animación Lottie JSON.

```
rlvgl-creator lottie cli [--bin PATH] <json> <out> [--apng FILE]
```
* `--bin` – binario externo (predeterminado `lottie-cli`).
* `json` – ruta al archivo Lottie JSON.
* `out` – directorio donde se escriben los cuadros.
* `--apng` – archivo APNG opcional para generar.

### svg
Renderiza un SVG en archivos de imagen brutos.

```
rlvgl-creator svg <svg> <out> [--dpi DPI...] [--threshold VAL]
```
* `svg` – ruta al archivo SVG.
* `out` – directorio donde se escriben las imágenes en bruto.
* `--dpi` – uno o más valores de DPI para renderizar (predeterminado `96`).
* `--threshold` – umbral monocromático (0–255).

### bsp from-ioc
Renderiza código fuente Rust a partir de un proyecto CubeMX usando una plantilla MiniJinja.

```
rlvgl-creator bsp from-ioc <ioc> [--emit-hal] [--emit-pac] [--template <template>]
    --out <dir> [--grouped-writes] [--one-file | --per-peripheral] [--with-deinit]
    [--allow-reserved]
```
* `ioc` – archivo `.ioc` de entrada de CubeMX.
* `--emit-hal` – renderiza usando la plantilla HAL incorporada.
* `--emit-pac` – renderiza usando la plantilla PAC incorporada.
* `--template` – ruta a una plantilla MiniJinja personalizada.
* `--out` – directorio para colocar el archivo fuente generado.
* `--grouped-writes` – colapsa las escrituras RCC por registro.
  Selecciona automáticamente nombres de bus específicos de la familia a través de F0, F1, F2,
  F3, F4, F7, G0, G4, H5, H7, L0, L1, L4, L5, U5, WB, y WL.
* `--one-file` – emite un único archivo fuente consolidado.
* `--per-peripheral` – emite un archivo por periférico con exclusión de características.
* `--with-deinit` – incluye ayudantes opcionales de desinicialización.
* `--allow-reserved` – permite la configuración de pines SWD reservados (`PA13`, `PA14`).
  Los ayudantes controlan los relojes, enmascaran las IRQ y restablecen la configuración de DMA/BDMA/MDMA
  registros, incluyendo el enrutamiento de DMAMUX y casos de borde de flujo/canal.
  Cubre controladores en variantes F0, F1, F2, F3, F4, F7, H5, H7, L0, L1, L4,
  L5, G0, G4, U5, WB, y WL.

Ver también: Comportamiento, banderas y hoja de ruta de generación de BSP de STM32 en
[STM_BSP_GENERATION.md](./STM_BSP_GENERATION.md) — incluye división de doble núcleo,
potencia (SCUEN/VOS) y detalles de reloj (PLL1/prescaladores).

#### Ejemplos de configuración avanzada

Genera BSPs HAL y PAC con escrituras RCC agrupadas, diseño por periférico y ganchos de desinicialización:

```bash
rlvgl-creator bsp from-ioc f769.ioc \
    --emit-hal --emit-pac --grouped-writes \
    --per-peripheral --with-deinit --out bsp
```

Renderiza un BSP mínimo solo PAC en un solo archivo para una puesta en marcha temprana:

```bash
rlvgl-creator bsp from-ioc bringup.ioc \
    --emit-pac --one-file --out bsp
```

Genera un BSP solo HAL con escrituras RCC no agrupadas en un solo archivo:

```bash
rlvgl-creator bsp from-ioc minimal.ioc \
    --emit-hal --one-file --out bsp
```

Recorrido por un BSP STM32F769I-DISCO consciente del bus con limpieza completa de DMA:

1. Genera código HAL y PAC con escrituras agrupadas, diseño por periférico y ganchos de desinicialización:
   ```bash
   rlvgl-creator bsp from-ioc f769.ioc \
       --emit-hal --emit-pac --grouped-writes \
       --per-peripheral --with-deinit --out bsp
   ```
2. Llama a `board::deinit()` durante el apagado para controlar los relojes, enmascarar las interrupciones y restablecer el estado de DMA/BDMA/MDMA.

Recorrido por un BSP STM32H573I-DISCO consciente del bus con escrituras no agrupadas:

 1. Genera código HAL en un solo archivo sin escrituras RCC agrupadas:
  ```bash
  rlvgl-creator bsp from-ioc h573.ioc \
      --emit-hal --one-file --out bsp
  ```
2. Llama a `board::deinit()` durante el apagado para controlar los relojes y restablecer el estado de los pines.

### Casos extremos y trampas

* Los registros de reloj periférico varían entre familias de bajo consumo como L0 y
  L1. Revisa las escrituras RCC generadas al apuntar a piezas recién agregadas.
* La limpieza de DMA borra los canales DMAMUX y los registros de flujo, pero aún no
  maneja los modos de lista enlazada o doble búfer.
* Algunos periféricos requieren pasos de reinicio adicionales más allá del control de reloj; verifica
  los ganchos de desinicialización para bloques IP personalizados o raros.

## Flujo de trabajo: de STM32 `.ioc` a BSP
1. Convierte el archivo `.ioc` a una representación intermedia y renderiza un crate BSP (AFs derivados de datos de proveedores embebidos):
   ```bash
   rlvgl-creator bsp from-ioc simple.ioc --emit-hal --out bsp
   ```
2. Usa el BSP generado en un proyecto:
   ```rust
   // Cargo.toml
   // [dependencies]
   // board = { path = "bsp" }

   // main.rs
   board::init();
   ```

## Flujo de trabajo: crear y finalizar una biblioteca de activos
1. Inicializa un nuevo paquete y registra un objetivo `host`:
   ```bash
   rlvgl-creator init
   rlvgl-creator add-target host vendor
   ```
2. Añade activos brutos:
   * Coloca los archivos de imagen en `icons/` o `media/`.
   * Copia las fuentes (`.ttf`, `.otf`) en `fonts/`.
3. Escanea y convierte los activos:
   ```bash
   rlvgl-creator scan assets/
   rlvgl-creator convert assets/
   ```
4. Genera previsualizaciones y sincroniza las listas de características:
   ```bash
   rlvgl-creator preview assets/
   rlvgl-creator sync vendor
   ```
5. Estructura un crate que exponga los activos:
   ```bash
   rlvgl-creator scaffold assets-pack
   ```
6. Usa el crate de activos:
   ```rust
   // Cargo.toml
   // [dependencies]
   // assets_pack = { path = "assets-pack" }

   // main.rs
   use assets_pack::fonts::PRIMARY_FONT;
   use assets_pack::images::LOGO;
   ```
   El crate proporciona accesores fuertemente tipados para fuentes y gráficos que pueden ser incrustados o vendidos dependiendo de las características de construcción.

## Ejemplos de uso
* **BSP** – Incluye el crate de placa generado en un proyecto de firmware y llama a `board::init()` para configurar relojes y multiplexación de pines.
* **Biblioteca de activos** – Depende del crate de activos estructurado y referencia los elementos exportados como `assets_pack::images::LOGO` al construir widgets.
```
