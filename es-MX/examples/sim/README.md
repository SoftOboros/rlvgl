<!--
examples/sim/README.md - Desktop simulator example.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Demostración de rlvgl
---
Demuestra los widgets principales junto con las características del plugin, como la generación de códigos QR
y la decodificación de imágenes PNG/JPEG.

## Uso

Ejecute el simulador con una resolución de pantalla personalizada usando:

```bash
cargo run --bin rlvgl-sim -- --screen=800x480
```

Omita `--screen` para usar la resolución predeterminada de 320x240. Por defecto, el simulador
usa el blitter de respaldo de la CPU para renderizar. Pase `--wgpi` para habilitar el blitter
acelerado por wgpu en su lugar. Proporcione una ruta de archivo como argumento adicional para
exportar un solo fotograma a un PNG en lugar de iniciar la ventana interactiva.

Para flujos de trabajo de gestión de activos usando `rlvgl-creator`, vea
[`README-CREATOR.md`](../../README-CREATOR.md).

## Limitaciones

En pantallas que exceden el tamaño máximo de textura de la GPU, el simulador
renderiza a un framebuffer interno más pequeño y escala el resultado para ajustarse a la
ventana. Este escalado puede introducir bandas negras o una nitidez reducida en
monitores de ultra alta resolución.

## Requisitos
La demostración de rlvgl requiere libgtk-3-dev y librlottie-dev para la visualización y el soporte de la creación de Lottie (no implementado).

Vea [Dockerfile](../../Dockerfile) y [setup-ci-env.sh](../../scripts/setup-ci-env.sh) para comprender el conjunto completo de paquetes utilizados para la ejecución.

Si no está disponible, rlottie se puede construir desde la fuente de la siguiente manera:
```bash
# Establecer la ruta del prefijo de instalación (modificar según sea necesario)
INSTALL_PREFIX="/opt/rlottie"

# Construir e instalar rlottie localmente
git clone https://github.com/Samsung/rlottie
cd rlottie && mkdir build && cd build
cmake .. \
    -DCMAKE_C_COMPILER=clang \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DCMAKE_INSTALL_PREFIX="$INSTALL_PREFIX" \
    -DLIB_INSTALL_DIR=lib \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5
make -j"$(nproc)"
make install
cd ../..

# Exportar variables de entorno a pasos futuros
export PKG_CONFIG_PATH="$INSTALL_PREFIX/lib/pkgconfig"
export BINDGEN_EXTRA_CLANG_ARGS="-I$INSTALL_PREFIX/include"

```

---

## Configuración de VS Code

### Plugins
- [CodeLLDB](https://github.com/vadimcn/codelldb)
- [rust-analyzer](https://rust-analyzer.github.io)
- Even Better TOML

### Configuración de lanzamiento
(.vscode/launch.json)[../../../.vscode/launch.json] contiene la configuración para ejecutar en macOS en x86

```json
{
  "version": "0.2.0",
  "configurations": [

    {
      "name": "Debug sim",
      "type": "lldb",
      "request": "launch",
      "program": "${workspaceFolder}/target/x86_64-apple-darwin/debug/rlvgl-sim",
      "args": [],
      "cwd": "${workspaceFolder}",
      "cargo": {
        "args": ["build", "--package=rlvgl-sim", "--target=x86_64-apple-darwin"]
      },
      "sourceLanguages": ["rust"]
    },
  ]
}
```

Cambie la cadena de destino para su plataforma host.
