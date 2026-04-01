<!--
README.md - Usage and format notes for the rlvgl-chips-nrf vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-nrf
Paquete: `rlvgl-chips-nrf`

Proporciona una base de datos de placas para dispositivos Nordic Semiconductor utilizada por `rlvgl-creator`.

## Uso

Este crate espera archivos de definición de placa extraídos por [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Durante la compilación, configure la
variable de entorno `RLVGL_CHIP_SRC` al directorio que contiene esos
archivos extraídos:

```sh
RLVGL_CHIP_SRC=build/chipdb/nrf cargo build -p rlvgl-chips-nrf
```

La biblioteca expone funciones de ayuda para los consumidores:

- `vendor()` – devuelve `"nrf"`.
- `boards()` – lista las placas soportadas como entradas `BoardInfo`.
- `find(name)` – busca una placa por su nombre exacto.

`rlvgl-creator` integra este crate para llenar los menús desplegables de proveedores y placas.
Otros crates de proveedores siguen el mismo diseño y API.

## Formato BoardInfo

Cada `BoardInfo` describe una placa con al menos un nombre de placa fácil de usar
y el chip asociado. Futuras versiones pueden incluir información de paquetes y
desplazamientos de configuración de pines.

## Características

- Soporte opcional de `serde` para serializar la base de datos de la placa: habilite la
  característica `serde` si la integración con herramientas externas lo requiere.
