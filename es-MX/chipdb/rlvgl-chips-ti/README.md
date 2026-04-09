<!--
README.md - Usage and format notes for the rlvgl-chips-ti vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-ti
Paquete: `rlvgl-chips-ti`

Proporciona una base de datos de placas para dispositivos Texas Instruments utilizada por `rlvgl-creator`.

## Uso

Este crate espera archivos de definición de placa extraídos por [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Durante la compilación, configure la variable de entorno `RLVGL_CHIP_SRC` al directorio que contiene esos archivos extraídos:

```sh
RLVGL_CHIP_SRC=build/chipdb/ti cargo build -p rlvgl-chips-ti
```

La librería expone funciones de ayuda para los consumidores:

- `vendor()` – devuelve `"ti"`.
- `boards()` – lista las placas soportadas como entradas `BoardInfo`.
- `find(name)` – busca una placa por su nombre exacto.

`rlvgl-creator` integra este crate para poblar los menús desplegables de proveedores y placas. Otros crates de proveedores siguen el mismo diseño y API.

## Formato BoardInfo

Cada `BoardInfo` describe una placa con al menos un nombre de placa fácil de usar y el chip asociado. Las versiones futuras pueden incluir información del paquete y compensaciones de configuración de pines.

## Características

- Soporte opcional de `serde` para serializar la base de datos de la placa: habilite la característica `serde` si la integración con herramientas externas lo requiere.
