/* ESP32-C3 ESP-HAL linker fragment: keep the ESP-IDF application descriptor at the start of the first DROM
 * segment. esp-hal 1.0.0-beta.0 predates this section in its linker script,
 * but current ESP-IDF second-stage bootloaders require it there. */
SECTIONS
{
  .rodata_desc : ALIGN(4)
  {
    KEEP(*(.rodata_desc.appdesc))
  } > RODATA
}
INSERT AFTER .text_dummy;
