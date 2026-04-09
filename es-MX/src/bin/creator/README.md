<!--
src/bin/creator/README.md - Guide to the rlvgl-creator binary workflows.
-->
<p align="center">
  <img src="../../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator

Una herramienta combinada de interfaz de usuario y línea de comandos para normalizar activos y generar crates de activos de modo dual para proyectos rlvgl. Ejecutar sin argumentos inicia la interfaz de usuario de escritorio; proporcionar argumentos ejecuta la CLI. Esta guía cubre el flujo de trabajo de principio a fin, desde la inicialización hasta el consumo.

## Flujo de trabajo

1.  **Inicializar carpetas y manifiesto**
    ```sh
    cargo run --bin rlgvl-creator --features creator,creator_ui -- init
    ```
    Crea `icons/`, `fonts/`, `media/` y un `manifest.yml` en el directorio de trabajo.

2.  **Escanear nuevos activos o activos modificados**
    ```sh
    cargo run --bin rlgvl-creator --features creator,creator_ui -- scan .
    ```
    Actualiza los hashes en el manifiesto para los activos bajo las raíces permitidas.

3.  **Convertir activos en secuencias sin procesar y paquetes de fuentes**
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- convert
    ```
    Las imágenes rasterizadas se convierten en secuencias RGBA sin procesar, y las fuentes se empaquetan en binarios de mapa de bits y métricas. Las conversiones se ejecutan en paralelo con un orden estable. Use `--force` para reconstruir todos los activos independientemente de la caché.

    Para renderizar activos vectoriales, el comando `svg` convierte un SVG en una o más imágenes sin procesar con los valores de DPI elegidos:
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- svg logo.svg out/ --dpi 96 --dpi 192
    ```
    Proporcione `--threshold <VAL>` para aplicar un corte monocromático adecuado para pantallas de tinta electrónica.

4.  **Sincronizar banderas de características, constantes e índice**
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- sync
    ```
    Regenera el código impulsado por el manifiesto sin tocar los bytes del activo.

5.  **Andamiar un crate de activos de consumidor**
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- scaffold assets-crate
    ```
    Genera un crate con características `embed` y `vendor` que expone sus activos procesados.

6.  **Activos de proveedor para la salida de compilación**
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- vendor
    ```
    Copia los activos procesados a `$OUT_DIR` y emite un módulo `rlvgl_assets.rs` para su inclusión.

El crate resultante se puede compilar con `--features embed` para incluir bytes sin procesar o `--features vendor` para copiar archivos en tiempo de compilación mientras se importa el módulo generado.

## Interfaz de usuario de escritorio y emulador

Inicie la interfaz de usuario de escritorio explícitamente:

```sh
cargo run --bin rlvgl-creator --features creator,creator_ui -- ui
```

Ejecute el simulador desde el mismo binario:

```sh
cargo run --bin rlvgl-creator --features creator,creator_ui -- sim --screen=800x480 --png --qrcode
```

## Notas del desarrollador

Para obtener detalles sobre cómo personalizar las plantillas de andamiaje y extender la tubería de conversión, consulte
[`docs/CREATOR-TEMPLATES.md`](../../../docs/CREATOR-TEMPLATES.md).
