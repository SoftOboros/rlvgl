/* memory.x (root) - Memory regions for STM32H747I-DISCO.
 *
 * Placed at crate root so cortex-m-rt's link.x can `INCLUDE memory.x`
 * without relying on custom search paths.
 */

MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
  /* STM32H747 CM7 DTCM RAM = 128K at 0x2000_0000 */
  RAM   : ORIGIN = 0x20000000, LENGTH = 128K
}

REGION_ALIAS("REGION_TEXT",   FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);
REGION_ALIAS("REGION_DATA",   RAM);
REGION_ALIAS("REGION_BSS",    RAM);
REGION_ALIAS("REGION_HEAP",   RAM);
REGION_ALIAS("REGION_STACK",  RAM);
