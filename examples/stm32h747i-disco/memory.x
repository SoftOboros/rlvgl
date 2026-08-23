/* memory.x - Memory regions for STM32H747I-DISCO.
 *
 * This file intentionally defines ONLY memory layout and leaves section
 * placement to cortex-m-rt's `link.x`. That script provides the vector table
 * and correct entry point (`Reset`) so the CPU boots from Flash properly.
 */

MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 1024K  /* Bank 1 (CM7 only) */
  /* STM32H747 CM7 DTCM RAM = 128K at 0x2000_0000 */
  RAM   : ORIGIN = 0x20000000, LENGTH = 128K
  /* D1 AXI SRAM total 512K @ 0x2400_0000: split 3/4:1/4 between CM7 and CM4 */
  D1_CM7  : ORIGIN = 0x24000000, LENGTH = 384K
  D1_CM4  : ORIGIN = 0x24060000, LENGTH = 128K
  /* Reserve a 1K cross-core mailbox in D2 SRAM3 for CM7<->CM4 */
  MAILBOX : ORIGIN = 0x30047000, LENGTH = 1K
  /* D3 SRAM4 (64K) is owned by CM4 for low-power retention; declared for visibility */
  D3_CM4  : ORIGIN = 0x38000000, LENGTH = 64K
  /* External QSPI flash (MT25TL01G Bank 1, 64 MB) memory-mapped via QUADSPI */
  QSPI_FLASH : ORIGIN = 0x90000000, LENGTH = 64M
  /* External SDRAM connected via FMC Bank1 (32 MB typical on H747I-DISCO) */
  SDRAM   : ORIGIN = 0xC0000000, LENGTH = 32M
}

/* Optional aliases used by newer link.x scripts; harmless if unused. */
REGION_ALIAS("REGION_TEXT",   FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);
REGION_ALIAS("REGION_DATA",   RAM);
REGION_ALIAS("REGION_BSS",    RAM);
REGION_ALIAS("REGION_HEAP",   RAM);
REGION_ALIAS("REGION_STACK",  RAM);

/* QSPI flash linker region helpers (memory-mapped at 0x9000_0000) */
PROVIDE(_qspi_flash_start = ORIGIN(QSPI_FLASH));
PROVIDE(_qspi_flash_size  = LENGTH(QSPI_FLASH));

/* SDRAM linker region helpers (for allocators or optional sections) */
PROVIDE(_sdram_start = ORIGIN(SDRAM));
PROVIDE(_sdram_size  = LENGTH(SDRAM));

/* Optional custom section aliases for future placement */
/* Place `.axisram_cm7` into D1_CM7 if referenced in a custom linker script */
PROVIDE(_axisram_cm7_start = ORIGIN(D1_CM7));
PROVIDE(_axisram_cm7_size  = LENGTH(D1_CM7));
/* Export the current legacy-demo mailbox candidate into every linked image.
 * MPY-08 does not treat this placement as normative until its platform PCDNs
 * are resolved; these symbols make the existing candidate mechanically
 * auditable in paired-image evidence. */
_mailbox_base = ORIGIN(MAILBOX);
_mailbox_size = LENGTH(MAILBOX);
/* Place `.retained_d3` into D3_CM4 if required */
PROVIDE(_retained_d3_start = ORIGIN(D3_CM4));
PROVIDE(_retained_d3_size  = LENGTH(D3_CM4));

/* Flash image size: from start of FLASH to end of last loaded section.
 * cortex-m-rt places .vector_table + .text + .rodata + .data(LMA) in FLASH.
 * __edata is the end of .data LMA (= end of flash image).
 * If .data is empty, __sidata == __edata and the total is just .text+.rodata. */
PROVIDE(_flash_image_end = __sidata + (__edata - __sdata));

/* Preserve reset breadcrumbs and keep the bounded renderer scratch away from
 * the downward-growing DTCM stack. Both are runtime-owned storage and must not
 * become loadable flash payloads. */
SECTIONS
{
  .noinit (NOLOAD) : ALIGN(4)
  {
    *(.noinit .noinit.*)
  } > RAM

  .rlvgl_blit_scratch (NOLOAD) : ALIGN(4)
  {
    *(.rlvgl_blit_scratch .rlvgl_blit_scratch.*)
  } > D1_CM7
} INSERT AFTER .uninit;
