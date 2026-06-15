/*
 * rlvgl_app.h - C ABI for the Rust rlvgl staticlib payload (BEETLE M1).
 *
 * Implemented by components/rlvgl_app/rust (librlvgl_app.a). The IDF host owns
 * all DSI/DPI/PSRAM hardware bring-up; this entry point draws an rlvgl widget
 * tree into the DPI RGB888 framebuffer.
 */
#ifndef RLVGL_APP_H
#define RLVGL_APP_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Build and draw one rlvgl screen into a 24-bit packed RGB framebuffer.
 *
 *   fb      - pointer to width * height * 3 writable bytes (R,G,B per pixel)
 *   width   - framebuffer width in pixels
 *   height  - framebuffer height in pixels
 *
 * The caller owns cache coherency: call esp_cache_msync(fb, ..., C2M) after
 * this returns, exactly as the original solid-color fill did. The call does
 * not block and does no hardware access of its own.
 */
void rlvgl_app_render(uint8_t *fb, int32_t width, int32_t height);

#ifdef __cplusplus
}
#endif

#endif /* RLVGL_APP_H */
