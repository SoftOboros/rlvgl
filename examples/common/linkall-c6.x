/* linkall-c6.x - ESP32-C6 linker root compatible with esp-hal 1.0.0-beta.0.
 *
 * The beta HAL orders executable flash before read-only flash. Current
 * ESP-IDF bootloaders require esp_app_desc at application offset 0x20 and
 * accept at most two cache-mapped segments. Keep descriptor and constants in
 * the first segment, then align executable flash into the second segment.
 */

INCLUDE "memory.x"

REGION_ALIAS("ROTEXT", ROM);
REGION_ALIAS("RODATA", ROM);
REGION_ALIAS("RWTEXT", RAM);
REGION_ALIAS("RWDATA", RAM);
REGION_ALIAS("RTC_FAST_RWTEXT", RTC_FAST);
REGION_ALIAS("RTC_FAST_RWDATA", RTC_FAST);

ENTRY(_start)

PROVIDE(_stext = ORIGIN(ROTEXT));
PROVIDE(_max_hart_id = 0);

PROVIDE(UserSoft = DefaultHandler);
PROVIDE(SupervisorSoft = DefaultHandler);
PROVIDE(MachineSoft = DefaultHandler);
PROVIDE(UserTimer = DefaultHandler);
PROVIDE(SupervisorTimer = DefaultHandler);
PROVIDE(MachineTimer = DefaultHandler);
PROVIDE(UserExternal = DefaultHandler);
PROVIDE(SupervisorExternal = DefaultHandler);
PROVIDE(MachineExternal = DefaultHandler);
PROVIDE(ExceptionHandler = DefaultExceptionHandler);
PROVIDE(interrupt0 = DefaultHandler);
PROVIDE(__post_init = default_post_init);
PROVIDE(_setup_interrupts = default_setup_interrupts);
PROVIDE(_mp_hook = default_mp_hook);
PROVIDE(_start_trap = default_start_trap);
PROVIDE(__global_pointer$ = _data_start + 0x800);

SECTIONS
{
  .trap : ALIGN(4)
  {
    KEEP(*(.trap));
    *(.trap.*);
  } > RWTEXT

  .flash.appdesc : ALIGN(4)
  {
    KEEP(*(.flash.appdesc));
    KEEP(*(.flash.appdesc.*));
  } > RODATA

  .rodata : ALIGN(4)
  {
    . = ALIGN(4);
    _rodata_start = ABSOLUTE(.);
    *(.rodata .rodata.*)
    *(.srodata .srodata.*)
    . = ALIGN(4);
    _rodata_end = ABSOLUTE(.);
  } > RODATA

  .rodata.wifi : ALIGN(4)
  {
    . = ALIGN(4);
    *(.rodata_wlog_*.*)
    . = ALIGN(4);
  } > RODATA

  .espressif.metadata : ALIGN(4)
  {
    KEEP(*(.espressif.metadata));
  } > RODATA
}

INCLUDE "rwtext.x"

SECTIONS
{
  .c6_text_gap (NOLOAD) :
  {
    . = . + 8;
    . = ALIGN(0x10000) + 0x20;
  } > ROM
}

INCLUDE "text.x"
INCLUDE "rwdata.x"
INCLUDE "rtc_fast.x"
INCLUDE "stack.x"
INCLUDE "dram2.x"
INCLUDE "debug.x"

_dram_origin = ORIGIN(RAM);

ASSERT(ADDR(.flash.appdesc) == ORIGIN(ROM),
       "ESP32-C6 app descriptor must start at the flash ROM origin");
ASSERT(SIZEOF(.flash.appdesc) == 0x100,
       "ESP32-C6 app descriptor must be exactly 256 bytes");
ASSERT((ADDR(.text) & 0xffff) == 0x20,
       "ESP32-C6 executable flash must start at MMU page offset 0x20");

INCLUDE "hal-defaults.x"
INCLUDE "rom-functions.x"
