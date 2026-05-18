/*
 * Minimal <string.h> stub for FreeRTOS on bare-metal.
 *
 * FreeRTOS calls memcpy / memset / memcmp for queue payload handling.
 * The Rust compiler-builtins crate provides these symbols at link time,
 * so we only need the prototypes here.
 */
#ifndef _RLVGL_FREERTOS_STRING_H
#define _RLVGL_FREERTOS_STRING_H

#include <stddef.h>

void *memcpy(void *dst, const void *src, size_t n);
void *memmove(void *dst, const void *src, size_t n);
void *memset(void *s, int c, size_t n);
int   memcmp(const void *a, const void *b, size_t n);
size_t strlen(const char *s);

#endif
