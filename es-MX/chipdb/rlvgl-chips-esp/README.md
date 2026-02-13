<!--
README.md - Notas de uso y formato para el crate de proveedor rlvgl-chips-esp.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-esp
Paquete: `rlvgl-chips-esp`

Proporciona una base de datos de placas para dispositivos Espressif utilizada por `rlvgl-creator`.

## Uso

Este crate espera archivos de definición de placas extraídos por [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Durante la compilación, establezca la
variable de entorno `RLVGL_CHIP_SRC` en el directorio que contiene esos
archivos extraídos:

```sh
RLVGL_CHIP_SRC=build/chipdb/esp cargo build -p rlvgl-chips-esp
```

La biblioteca expone funciones de ayuda para los consumidores:

- `vendor()` – devuelve `"esp"`.
- `boards()` – lista las placas compatibles como entradas de `BoardInfo`.
- `find(name)` – busca una placa por su nombre exacto.

`rlvgl-creator` integra este crate para llenar los menús desplegables de proveedores y placas.
Otros crates de proveedores siguen el mismo diseño y API.

## Formato BoardInfo

Cada `BoardInfo` describe una placa con al menos un nombre de placa fácil de usar
y el chip asociado. Las versiones futuras pueden incluir información del paquete y
desplazamientos de configuración de pines.

## Características

- Soporte opcional de `serde` para serializar la base de datos de placas: habilite la
  característica `serde` si la integración con herramientas externas lo requiere.
