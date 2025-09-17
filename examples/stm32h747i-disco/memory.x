/* memory.x - Memory regions for STM32H747I-DISCO.
 *
 * This file intentionally defines ONLY memory layout and leaves section
 * placement to cortex-m-rt's `link.x`. That script provides the vector table
 * and correct entry point (`Reset`) so the CPU boots from Flash properly.
 */

MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
  /* STM32H747 CM7 DTCM RAM = 128K at 0x2000_0000 */
  RAM   : ORIGIN = 0x20000000, LENGTH = 128K
  /* D1 AXI SRAM total 512K @ 0x2400_0000: split 3/4:1/4 between CM7 and CM4 */
  D1_CM7  : ORIGIN = 0x24000000, LENGTH = 384K
  D1_CM4  : ORIGIN = 0x24060000, LENGTH = 128K
  /* Reserve a 1K cross-core mailbox in D2 SRAM3 for CM7<->CM4 */
  MAILBOX : ORIGIN = 0x30047000, LENGTH = 1K
  /* D3 SRAM4 (64K) is owned by CM4 for low-power retention; declared for visibility */
  D3_CM4  : ORIGIN = 0x38000000, LENGTH = 64K
}

/* Optional aliases used by newer link.x scripts; harmless if unused. */
REGION_ALIAS("REGION_TEXT",   FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);
REGION_ALIAS("REGION_DATA",   RAM);
REGION_ALIAS("REGION_BSS",    RAM);
REGION_ALIAS("REGION_HEAP",   RAM);
REGION_ALIAS("REGION_STACK",  RAM);

/* Optional custom section aliases for future placement */
/* Place `.axisram_cm7` into D1_CM7 if referenced in a custom linker script */
PROVIDE(_axisram_cm7_start = ORIGIN(D1_CM7));
PROVIDE(_axisram_cm7_size  = LENGTH(D1_CM7));
/* Place `.mailbox` into MAILBOX region if a custom script includes it */
PROVIDE(_mailbox_base = ORIGIN(MAILBOX));
PROVIDE(_mailbox_size = LENGTH(MAILBOX));
/* Place `.retained_d3` into D3_CM4 if required */
PROVIDE(_retained_d3_start = ORIGIN(D3_CM4));
PROVIDE(_retained_d3_size  = LENGTH(D3_CM4));
