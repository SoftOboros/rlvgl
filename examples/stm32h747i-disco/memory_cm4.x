/* memory_cm4.x - Memory regions for STM32H747I-DISCO CM4 core.
 *
 * CM4 executes from Flash but typically uses D2 SRAM (SRAM1/2/3) at 0x3000_0000.
 * This script assigns 256K to CM4. Adjust as needed for your application.
 */

MEMORY
{
  FLASH : ORIGIN = 0x08100000, LENGTH = 1024K  /* Bank 2 (RM0399 BCM4_ADD0 default) */
  /* D2 SRAM (SRAM1/2/3) starts at 0x3000_0000; allocate 256K for CM4 */
  RAM   : ORIGIN = 0x30000000, LENGTH = 256K
  /* Reserve a 1K cross-core mailbox in D2 SRAM3 for CM7<->CM4 */
  MAILBOX : ORIGIN = 0x30047000, LENGTH = 1K
  /* D1 AXI SRAM total 512K @ 0x2400_0000: split 3/4:1/4; CM4 gets the upper 128K */
  D1_CM7  : ORIGIN = 0x24000000, LENGTH = 384K
  D1_CM4  : ORIGIN = 0x24060000, LENGTH = 128K
  /* D3 SRAM4 (64K) fully owned by CM4 for low-power retention */
  D3_CM4  : ORIGIN = 0x38000000, LENGTH = 64K
}

REGION_ALIAS("REGION_TEXT",   FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);
REGION_ALIAS("REGION_DATA",   RAM);
REGION_ALIAS("REGION_BSS",    RAM);
REGION_ALIAS("REGION_HEAP",   RAM);
REGION_ALIAS("REGION_STACK",  RAM);

/* Optional custom section aliases for future placement */
/* Place `.axisram_cm4` into D1_CM4 if referenced in a custom linker script */
PROVIDE(_axisram_cm4_start = ORIGIN(D1_CM4));
PROVIDE(_axisram_cm4_size  = LENGTH(D1_CM4));
/* Export the same legacy-demo mailbox candidate as the CM7 image so paired
 * link evidence can reject divergent base or extent values. */
_mailbox_base = ORIGIN(MAILBOX);
_mailbox_size = LENGTH(MAILBOX);
ASSERT((ORIGIN(MAILBOX) & 31) == 0, "legacy mailbox must be 32-byte aligned");
ASSERT(LENGTH(MAILBOX) >= 0x2A8, "legacy mailbox is too small for paired queues");
/* Place `.retained_d3` into D3_CM4 if required */
PROVIDE(_retained_d3_start = ORIGIN(D3_CM4));
PROVIDE(_retained_d3_size  = LENGTH(D3_CM4));
