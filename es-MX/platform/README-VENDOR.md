<!--
platform/README-VENDOR.md - Policy for vendor-specific platform support.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Política de Soporte para Proveedores para rlvgl

Este documento describe cómo se gestiona el soporte específico de proveedores en los crates de `rlvgl-platform`.
Aclara la distinción entre contribuciones de la comunidad y plataformas oficialmente soportadas.

---

## Principios Fundamentales

- La **librería principal `rlvgl`** es neutral respecto al proveedor y de código abierto.
- Los **crates `rlvgl-platform`** proporcionan capas de integración específicas para proveedores y placas.
- El soporte de plataforma se clasifica en niveles para reflejar diferentes grados de mantenimiento y garantías.

---

## Niveles de Soporte

### Soporte Oficial
- Mantenido directamente en el repositorio de `rlvgl`.
- Incluido en las compilaciones y pruebas de integración continua (CI).
- Documentado en los ejemplos y la galería oficiales.
- Compatibilidad garantizada con cada lanzamiento de `rlvgl`.
- Requiere el patrocinio del proveedor o un acuerdo de asociación equivalente.

### Soporte Comunitario
- Puede ser desarrollado y mantenido por colaboradores de la comunidad.
- Aceptado en el repositorio si pasa una revisión básica y compila.
- Compilado en CI solo para verificaciones en tiempo de compilación.
- No se garantiza su inclusión en la documentación o ejemplos.
- No hay garantía de compatibilidad entre versiones de `rlvgl`.

### Soporte Externo
- Desarrollado y mantenido fuera del repositorio de `rlvgl`.
- Puede ser enlazado desde la documentación como un recurso externo.
- Sin garantías ni responsabilidades por parte de los mantenedores de `rlvgl`.

---

## Participación del Proveedor

Los proveedores interesados en el **Soporte Oficial** deben proporcionar:
1. Patrocinio o asociación para cubrir el mantenimiento continuo.
2. Hardware de referencia (kits de evaluación, placas o módulos).
3. Documentación y material de prueba según sea necesario.

Esto asegura que el hardware del proveedor esté representado con la misma estabilidad, documentación y pulido que el simulador y otras plataformas oficialmente soportadas.

---

## Resumen

- **Cualquiera** puede desarrollar sobre `rlvgl` y contribuir con código de plataforma.
- El **estado oficial** está reservado para plataformas patrocinadas por proveedores, con cobertura completa de CI, ejemplos y documentación.
- Esta política mantiene el núcleo abierto al tiempo que asegura un soporte sostenible para los ecosistemas de los proveedores.
