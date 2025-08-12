/* Linker script for the STM32H747I-DISCO example. */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
  RAM : ORIGIN = 0x20000000, LENGTH = 1024K
}

ENTRY(main);

SECTIONS
{
  .text : {
    *(.text*)
    *(.rodata*)
    _etext = .;
  } > FLASH

  .data : AT (ADDR(.text) + SIZEOF(.text)) {
    _sdata = .;
    *(.data*)
    _edata = .;
  } > RAM

  .bss (NOLOAD) : {
    _sbss = .;
    *(.bss*)
    *(COMMON)
    _ebss = .;
  } > RAM
}
