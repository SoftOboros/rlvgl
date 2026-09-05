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
 * Build and draw one rlvgl screen into a 24-bit packed RGB framebuffer,
 * optionally overlaying a touch crosshair + coordinate readout.
 *
 *   fb            - pointer to width * height * 3 writable bytes (R,G,B/pixel)
 *   width         - framebuffer width in pixels
 *   height        - framebuffer height in pixels
 *   touch_x       - touch X in framebuffer pixels (ignored if !touch_active)
 *   touch_y       - touch Y in framebuffer pixels (ignored if !touch_active)
 *   touch_active  - nonzero to draw the touch marker at (touch_x, touch_y)
 *
 * The caller owns cache coherency: call esp_cache_msync(fb, ..., C2M) after
 * this returns, exactly as the original solid-color fill did. The call does
 * not block and does no display hardware access of its own.
 */
void rlvgl_app_render(uint8_t *fb, int32_t width, int32_t height,
                      int32_t touch_x, int32_t touch_y, int32_t touch_active);

/*
 * Start any payload-owned background service before display bring-up. This is
 * a no-op for the tutorial and disco payloads. The CCPS payload uses it to
 * start the read-only battery poller, which permits headless carrier bring-up.
 */
void rlvgl_app_init(void);

#ifdef __cplusplus
}
#endif

#endif /* RLVGL_APP_H */
