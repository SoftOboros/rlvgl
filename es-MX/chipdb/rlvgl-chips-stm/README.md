<!--
README.md - Usage and format notes for the rlvgl-chips-stm vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-stm
Paquete: `rlvgl-chips-stm`

Proporciona una base de datos de placas para dispositivos STMicroelectronics utilizados por `rlvgl-creator`.

## Uso

El crate publicado incrusta una base de datos de chips comprimida con zstd generada por `tools/build_vendor.sh`. Los consumidores simplemente enlazan el crate; el archivo se descomprime en tiempo de ejecución por `rlvgl-creator`.

Al construir desde una copia de git, ejecute primero `tools/build_vendor.sh` para producir el archivo `assets/chipdb.bin.zst` utilizado por el script de construcción:

```sh
VENDOR_DIR=chips/stm CRATE_DIR=chipdb/rlvgl-chips-stm OUT_DIR=build/chipdb/stm \
    bash tools/build_vendor.sh
```

Si `assets/chipdb.bin.zst` está ausente, el script de construcción recurre a la variable de entorno `RLVGL_CHIP_SRC` para localizar definiciones JSON sin comprimir.

Consulte [assets/README.md](./assets/README.md) para obtener detalles sobre el archivo de la base de datos comprimida.

La biblioteca expone funciones de ayuda para los consumidores:

- `vendor()` – devuelve `"stm"`.
- `boards()` – lista las placas compatibles como entradas `BoardInfo`.
- `find(name)` – busca una placa por su nombre exacto.

`rlvgl-creator` integra este crate para rellenar los menús desplegables de proveedores y placas. Otros crates de proveedores siguen el mismo diseño y API.

## Formato de BoardInfo

Cada `BoardInfo` describe una placa con al menos un nombre de placa legible por humanos y el chip asociado. Las versiones futuras pueden incluir información del paquete y compensaciones de configuración de pines.

## Características

- Soporte opcional de `serde` para serializar la base de datos de la placa: habilite la característica `serde` si la integración con herramientas externas lo requiere.
```
