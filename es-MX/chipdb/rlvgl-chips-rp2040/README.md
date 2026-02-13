<!--
README.md - Notas de uso y formato para el crate de proveedor rlvgl-chips-rp2040.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-rp2040
Paquete: `rlvgl-chips-rp2040`

Proporciona una base de datos de placas para dispositivos RP2040 genéricos utilizados por `rlvgl-creator`.

## Uso

Este crate espera archivos de definición de placa extraídos por [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Durante la compilación, configure la variable de entorno `RLVGL_CHIP_SRC` al directorio que contiene esos archivos extraídos:

```sh
RLVGL_CHIP_SRC=build/chipdb/rp2040 cargo build -p rlvgl-chips-rp2040
```

La librería expone funciones de ayuda para los consumidores:

- `vendor()` – devuelve `"rp2040"`.
- `boards()` – lista las placas soportadas como entradas `BoardInfo`.
- `find(name)` – busca una placa por su nombre exacto.

`rlvgl-creator` integra este crate para poblar los menús desplegables de proveedor y placa. Otros crates de proveedor siguen el mismo diseño y API.

## Formato de BoardInfo

Cada `BoardInfo` describe una placa con al menos un nombre de placa legible y el chip asociado. Las versiones futuras pueden incluir información del paquete y compensaciones de configuración de pines.

## Características

- Soporte opcional de `serde` para serializar la base de datos de placas: habilite la característica `serde` si la integración con herramientas externas lo requiere.
