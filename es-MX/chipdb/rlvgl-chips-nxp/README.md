```markdown
<!--
README.md - Usage and format notes for the rlvgl-chips-nxp vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-nxp
Paquete: `rlvgl-chips-nxp`

Proporciona una base de datos de placas para dispositivos NXP utilizados por `rlvgl-creator`.

## Uso

Esta crate espera archivos de definición de placa extraídos por [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Durante la compilación, configure la
variable de entorno `RLVGL_CHIP_SRC` al directorio que contiene esos
archivos extraídos:

```sh
RLVGL_CHIP_SRC=build/chipdb/nxp cargo build -p rlvgl-chips-nxp
```

La biblioteca expone funciones de ayuda para los consumidores:

- `vendor()` – devuelve `"nxp"`.
- `boards()` – lista las placas soportadas como entradas `BoardInfo`.
- `find(name)` – busca una placa por su nombre exacto.

`rlvgl-creator` integra esta crate para poblar los menús desplegables de proveedores y placas.
Otras crates de proveedores siguen el mismo diseño y API.

## Formato BoardInfo

Cada `BoardInfo` describe una placa con al menos un nombre de placa fácil de usar
y el chip asociado. Futuras versiones pueden incluir información del paquete y
desplazamientos de configuración de pines.

## Características

- Soporte opcional de `serde` para serializar la base de datos de la placa: habilite la
  característica `serde` si la integración con herramientas externas lo requiere.
```
