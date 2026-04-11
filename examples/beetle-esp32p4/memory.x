/* memory.x - Memory regions for the DFR1172 FireBeetle 2 ESP32-P4.
 *
 * Same style as examples/stm32h747i-disco/memory.x: this file defines ONLY
 * the MEMORY block and leaves section placement to esp-riscv-rt's `link.x`.
 *
 * All addresses are first-pass defaults sourced from the ESP32-P4 Technical
 * Reference Manual (memalpha notebook 15, doc esp32-p4_technical_reference_
 * manual_en.pdf). Verify against TRM chapter "System and Memory → Internal
 * Memory" and "External Memory → Cache Subsystem" before trusting them.
 */

MEMORY
{
  /* External flash cache window (QSPI/HSPI flash mapped into instruction
   * space via the FLASH MMU). DFR1172 ships with 16 MB of external flash. */
  /* TODO(TRM §Cache Subsystem): confirm ORIGIN for the instruction-view
   * flash cache window on ESP32-P4 (likely 0x42000000). */
  FLASH : ORIGIN = 0x42000000, LENGTH = 16M

  /* HP L2MEM data view — the 768 KB on-chip SRAM that backs .data/.bss/
   * .heap/.stack for the HP core. Leaving 128 KB of headroom for the
   * instruction side and MMU tables. */
  /* TODO(TRM §Internal Memory): confirm HP L2MEM data-view base and total
   * size on ESP32-P4 (likely 0x4FF00000, 768 KB). */
  RAM   : ORIGIN = 0x4FF00000, LENGTH = 640K

  /* Off-chip PSRAM, 32 MB on the DFR1172 per the FireBeetle 2 ESP32-P4
   * product spec. Useful for large framebuffers (1024×600×2 = 1.2 MB). */
  /* TODO(TRM §Cache Subsystem): confirm PSRAM data-view base on ESP32-P4. */
  PSRAM : ORIGIN = 0x48000000, LENGTH = 32M

  /* LP SRAM — 32 KB on-chip SRAM tied to the LP RISC-V core. Declared for
   * visibility even though the initial scaffold only uses the HP core. */
  /* TODO(TRM §LP Core): confirm LP SRAM base. */
  LPRAM : ORIGIN = 0x50108000, LENGTH = 32K
}

/* Aliases used by esp-riscv-rt's link.x; match the style in
 * examples/stm32h747i-disco/memory.x. */
REGION_ALIAS("REGION_TEXT",   FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);
REGION_ALIAS("REGION_DATA",   RAM);
REGION_ALIAS("REGION_BSS",    RAM);
REGION_ALIAS("REGION_HEAP",   RAM);
REGION_ALIAS("REGION_STACK",  RAM);

/* Linker helper symbols for PSRAM / LP SRAM placement by future custom
 * sections. */
PROVIDE(_psram_start = ORIGIN(PSRAM));
PROVIDE(_psram_size  = LENGTH(PSRAM));
PROVIDE(_lpram_start = ORIGIN(LPRAM));
PROVIDE(_lpram_size  = LENGTH(LPRAM));
