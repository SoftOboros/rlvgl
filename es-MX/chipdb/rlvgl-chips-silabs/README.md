<!--
README.md - Usage and format notes for the rlvgl-chips-silabs vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-silabs
Paquete: `rlvgl-chips-silabs`

Proporciona una base de datos de placas para dispositivos Silicon Labs utilizada por `rlvgl-creator`.

## Uso

Este crate espera archivos de definición de placa extraídos por [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Durante la construcción, establezca la
variable de entorno `RLVGL_CHIP_SRC` al directorio que contiene esos
archivos extraídos:

```sh
RLVGL_CHIP_SRC=build/chipdb/silabs cargo build -p rlvgl-chips-silabs
```

La biblioteca expone funciones auxiliares para los consumidores:

- `vendor()` – devuelve `"silabs"`.
- `boards()` – lista las placas soportadas como entradas `BoardInfo`.
- `find(name)` – busca una placa por su nombre exacto.

`rlvgl-creator` integra este crate para poblar los menús desplegables de proveedores y placas.
Otros crates de proveedores siguen el mismo diseño y API.

## Formato de BoardInfo

Cada `BoardInfo` describe una placa con al menos un nombre de placa fácil de usar
y el chip asociado. Futuras versiones pueden incluir información del paquete y compensaciones de configuración de pines.

## Características

- Soporte opcional de `serde` para serializar la base de datos de placas: habilite la
  característica `serde` si la integración con herramientas externas lo requiere.
```
