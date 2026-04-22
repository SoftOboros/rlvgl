/* Linker script for BeagleBone Black bare-metal (AM3358 + 512 MB DDR3L).
 *
 * U-Boot SPL initializes DDR3L, PLLs, and pinmux. U-Boot proper relocates
 * itself to the top of DDR (around 0x9F80_0000+ on a 512 MB board). The
 * standard AM335x `loadaddr` is 0x82000000, so we put our code there.
 *
 * DDR layout we assume after `go 0x82000000`:
 *   0x80000000..0x82000000  — 32 MB U-Boot scratch / SPL / bootargs
 *   0x82000000..0x83000000  — our .text + .rodata + .data + .bss (16 MB)
 *   0x83000000..0x84000000  — stack (grows down from __stack_top, 16 MB)
 *   0x84000000..0x84200000  — framebuffer (800*480*4 = 1.5 MB, rounded up)
 *   0x84200000..0x9F800000  — free
 *   0x9F800000..0xA0000000  — U-Boot image + heap
 *
 * The fixed 0x84000000 framebuffer is referenced from bare_metal.rs as
 * `FB_BASE` and passed into `lcdc::init_raster`. Keeping it as an address
 * literal in Rust (not in memory.x) keeps the same code path viable for
 * Linux, where the framebuffer lives in a /dev/mem-reserved region at a
 * different PA.
 */

MEMORY
{
    DDR  (rwx) : ORIGIN = 0x82000000, LENGTH = 16M
}

ENTRY(_start)

SECTIONS
{
    .text :
    {
        KEEP(*(.text._start))
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
        . = ALIGN(4);
        __bss_start = .;
        *(.bss .bss.* COMMON)
        . = ALIGN(4);
        __bss_end = .;
    } > DDR

    /* Stack grows down from __stack_top. Placed 16 MB past code origin so
     * stack frames never collide with .text/.rodata/.data/.bss regardless
     * of build profile (debug builds are much larger than release). */
    __stack_top = 0x84000000;

    /DISCARD/ :
    {
        *(.ARM.exidx .ARM.exidx.*)
        *(.ARM.extab .ARM.extab.*)
        *(.comment)
        *(.note .note.*)
    }
}
