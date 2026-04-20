/* Linker script for BeagleBone Black bare-metal (AM3358 + DDR3L)
 *
 * U-Boot SPL initializes DDR3L before chainloading our binary.
 * We place everything in DDR starting at 0x80000000.
 *
 * On-chip SRAM at 0x402F0400 (64 KB) is available without DDR
 * but too small for our framebuffer + widget tree.
 */

MEMORY
{
    DDR  (rwx) : ORIGIN = 0x80000000, LENGTH = 128M
    SRAM (rwx) : ORIGIN = 0x402F0400, LENGTH = 64K
}

ENTRY(_start)

SECTIONS
{
    .text :
    {
        *(.text._start)
        *(.text .text.*)
    } > DDR

    .rodata :
    {
        *(.rodata .rodata.*)
    } > DDR

    .data :
    {
        *(.data .data.*)
    } > DDR

    .bss (NOLOAD) :
    {
        __bss_start = .;
        *(.bss .bss.* COMMON)
        __bss_end = .;
    } > DDR

    /* Stack at end of first 2 MB of DDR */
    __stack_top = ORIGIN(DDR) + 0x200000;

    /DISCARD/ :
    {
        *(.ARM.exidx .ARM.exidx.*)
        *(.ARM.extab .ARM.extab.*)
    }
}
