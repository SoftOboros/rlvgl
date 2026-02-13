<!--
README.md - Usage and format notes for the rlvgl-chips-renesas vendor crate.
-->
<p align="centre">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-renesas
Paquete: `rlvgl-chips-renesas`

Proporciona una base de datos de placas para dispositivos Renesas utilizada por `rlvgl-creator`.

## Uso

Este crate espera archivos de definición de placa extraídos por [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Durante la compilación, configure la
variable de entorno `RLVGL_CHIP_SRC` al directorio que contiene esos
archivos extraídos:

```sh
RLVGL_CHIP_SRC=build/chipdb/renesas cargo build -p rlvgl-chips-renesas
```

La biblioteca expone funciones de ayuda para los consumidores:

- `vendor()` – devuelve `"renesas"`.
- `boards()` – lista las placas soportadas como entradas `BoardInfo`.
- `find(name)` – busca una placa por su nombre exacto.

`rlvgl-creator` integra este crate para llenar los menús desplegables de proveedor y placa.
Otros crates de proveedor siguen el mismo diseño y API.

## Formato BoardInfo

Cada `BoardInfo` describe una placa con al menos un nombre de placa fácil de usar
y el chip asociado. Las versiones futuras pueden incluir información del paquete y
desplazamientos de configuración de pines.

## Características

- Soporte opcional de `serde` para serializar la base de datos de placas: habilite la
  característica `serde` si la integración con herramientas externas lo requiere.
```
