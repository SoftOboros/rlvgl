```markdown
<!--
platform/README.md - Traits and utilities for la integración de hardware y simuladores.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-platform
Paquete: `rlvgl-platform`

Traits y tipos de utilidad para conectar rlvgl a hardware real o simuladores.

Se empareja con los crates [core](../core/README.md) y
[widgets](../widgets/README.md).

Consulte [README-VENDOR.md](./README-VENDOR.md) para conocer la política de soporte del proveedor.

Elementos proporcionados actualmente:

- Trait `DisplayDriver` para enviar datos de píxeles a un framebuffer o LCD
- Trait `InputDevice` para leer eventos de puntero o teclado
- Implementaciones dummy utilizadas para pruebas sin interfaz gráfica

## Backend stm32h747i_disco

La característica opcional `stm32h747i_disco` habilita controladores de pantalla y táctiles de marcador de posición para el panel MIPI-DSI y el controlador capacitivo FT5336 de la placa STM32H747I-DISCO. Estos stubs establecen la estructura del módulo para futuras integraciones de hardware.
```
