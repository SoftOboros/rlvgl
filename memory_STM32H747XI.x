/* memory_STM32H747XI.x - Linker memory script for STM32H747XI (Cortex‑M7 core)

   This script defines all internal RAM and Flash memory regions available in the STM32H747XI dual‑core MCU.
   It omits peripheral register blocks and backup SRAM.
*/

MEMORY
{
  ITCMRAM  (rx)  : ORIGIN = 0x00000000, LENGTH = 64K
  DTCMRAM  (xrw) : ORIGIN = 0x20000000, LENGTH = 128K
  RAM_D1   (xrw) : ORIGIN = 0x24000000, LENGTH = 512K
  RAM_D2   (xrw) : ORIGIN = 0x30000000, LENGTH = 288K
  RAM_D3   (xrw) : ORIGIN = 0x38000000, LENGTH = 64K
  FLASH    (rx)  : ORIGIN = 0x08000000, LENGTH = 2048K
}

ENTRY(main)

/* Choose which region each logical section should use: */
REGION_ALIAS("REGION_TEXT",   FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);

/* Put data/bss/stack/heap in RAM_D1 by default; override as needed */
REGION_ALIAS("REGION_DATA",   RAM_D1);
REGION_ALIAS("REGION_BSS",    RAM_D1);
REGION_ALIAS("REGION_HEAP",   RAM_D1);
REGION_ALIAS("REGION_STACK",  RAM_D1);

REGION_ALIAS("RAM",  RAM_D1);

/* Optional explicit stack start if you want */
PROVIDE(_stack_start = ORIGIN(RAM_D1) + LENGTH(RAM_D1));

/* Pull in cortex-m-rt’s section layout, vector table, KEEP rules, ENTRY(Reset), etc. */
INCLUDE link.x
