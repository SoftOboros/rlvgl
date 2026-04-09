```markdown
<!--
docs/TODO-CREATOR-UI-FULL.md - rlvgl-creator – TODO de Funcionalidad Completa de la UI.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator – TODO de Funcionalidad Completa de la UI

Este archivo rastrea el trabajo restante para que la interfaz de usuario de escritorio de `rlvgl-creator` tenga paridad con su CLI y proporcione una gestión completa de activos.

## Superficie de Comandos
- [x] Añadir un menú de comandos global que liste todas las acciones de la CLI con manejadores dedicados y retroalimentación (toast).
- [x] Exponer el comando `init` a través de un diálogo para crear raíces de activos y manifiesto predeterminado.
- [x] Añadir la acción `scan` con selector de directorio y actualización de manifiesto.
- [x] Añadir el comando `check` con selector de raíz y alternancia de reparación opcional.
- [x] Implementar la interfaz de usuario de la operación `vendor` para copiar activos y generar módulos de incrustación.
- [x] Exponer el comando `convert` con selector de raíz y bandera de forzado.
- [x] Añadir el comando `preview` para regenerar miniaturas bajo demanda.
- [x] Proporcionar un diálogo de registro `add-target` para el nombre y el directorio del proveedor.
- [x] Exponer el comando `sync` con directorio de salida y opción de simulación (dry-run).
- [x] Implementar la interfaz de usuario de `scaffold` para generar una crate de activos de modo dual.

## Herramientas de Conversión y Exportación
- [x] Expandir el constructor de APNG para permitir la configuración del retardo y el número de bucles; el directorio de fotogramas,
      la ruta de salida, el retardo y los bucles son configurables.
- [x] Añadir opción de exportación de esquema de manifiesto ejecutando `schema::run()`.
- [x] Exponer la interfaz de usuario del empaquetador de fuentes para el tamaño y el conjunto de caracteres; la ruta raíz,
      el tamaño y los glifos son configurables.
- [x] Integrar el importador Lottie (rutas CLI en proceso y externas).
 - [x] Añadir diálogo de renderizado SVG con lista de DPI configurable y umbral; ambas configuraciones son configurables por el usuario antes de renderizar.

## Navegador de Activos
- [x] Reemplazar la lista plana con un árbol jerárquico que refleje `assets/raw`; los directorios reflejan la jerarquía en disco.
- [x] Añadir acción "Añadir Activo" usando un diálogo de archivo para copiar archivos y actualizar el manifiesto
      (aún no hay flujo de trabajo de importación).
- [x] Permitir la eliminación de activos seleccionados con diálogo de confirmación y persistencia del manifiesto.
- [x] Mostrar el contenido completo del archivo con actualización automática cuando se añaden archivos externamente.
 
## Mejoras de Flujo de Trabajo y UX
- [x] Agrupar comandos relacionados en menús de nivel superior (Activos, Construir, Desplegar) para reemplazar el desorden de un botón por comando.
  - **Activos**: init, scan, check, vendor, convert, preview.
  - **Construir**: add-target, scaffold, exportación de esquema, paquete de fuentes, renderizado SVG.
  - **Desplegar**: sync, presets de automatización.
- [x] Introducir asistentes que guíen a través de secuencias comunes como escanear → convertir → previsualizar con indicación de progreso.
  - Pasos del asistente: seleccionar raíz → escanear activos → convertir formatos → previsualizar resultados → resumen.
- [x] Soportar presets de automatización o macros para encadenar comandos y repetir flujos de trabajo frecuentes.
  - Permitir guardar secuencias de comandos como presets nombrados en un archivo JSON y exponer un diálogo "Ejecutar Preset".
```
