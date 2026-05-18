/*
 * Minimal <stdlib.h> stub for FreeRTOS on bare-metal.
 *
 * The vendored homebrew arm-none-eabi-gcc does not ship newlib, so
 * FreeRTOS's `#include <stdlib.h>` would fail. FreeRTOS only needs
 * `size_t` and `NULL` from this header — both live in <stddef.h>,
 * which is a freestanding header provided by gcc itself.
 */
#ifndef _RLVGL_FREERTOS_STDLIB_H
#define _RLVGL_FREERTOS_STDLIB_H

#include <stddef.h>

#endif
