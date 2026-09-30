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
 * not block and does no hardware access of its own.
 */
void rlvgl_app_render(uint8_t *fb, int32_t width, int32_t height,
                      int32_t touch_x, int32_t touch_y, int32_t touch_active);

/*
 * Prepare one bounded Modbus RTU function-0x03 request. This is an offline
 * codec operation: it does not configure or access UART or GPIO hardware.
 * Returns 8 on success, -1 for a refused request, or -2 for a short/null output.
 */
int32_t rlvgl_ccps_modbus_prepare_read(uint8_t device_address,
                                      uint16_t human_register,
                                      uint16_t pdu_address,
                                      uint16_t quantity,
                                      uint8_t *out,
                                      uintptr_t out_capacity);

/*
 * Independent last-mile allowlist for an already encoded frame. Returns 1 only
 * for a CRC-valid, bounded function-0x03 device request; otherwise returns 0.
 * A future transport must call this immediately before asserting driver enable.
 */
int32_t rlvgl_ccps_modbus_wire_frame_is_allowed(const uint8_t *frame,
                                                uintptr_t frame_len);

/* Candidate evidence record. Raw bytes and decoded words stay together. */
typedef struct {
    uint8_t device_address;
    uint8_t evidence_grade; /* 0 = candidate; no parser can promote it */
    uint8_t raw_len;
    uint8_t register_count;
    uint16_t human_register;
    uint16_t pdu_address;
    uint64_t observed_at_ms;
    uint8_t raw[21];
    uint16_t registers[8];
    uint8_t exception_code;
} rlvgl_ccps_modbus_observation_t;

/*
 * Parse one exact response into an evidence record. Returns 1 for data, 2 for
 * a retained exception, or a negative refusal/error code. No hardware access.
 */
int32_t rlvgl_ccps_modbus_parse_response(
    uint8_t device_address, uint16_t human_register, uint16_t pdu_address,
    uint16_t quantity, const uint8_t *frame, uintptr_t frame_len,
    uint64_t observed_at_ms, rlvgl_ccps_modbus_observation_t *out);

/* Return 1 only if the observation is not stale under the supplied age bound. */
int32_t rlvgl_ccps_modbus_observation_is_fresh(
    const rlvgl_ccps_modbus_observation_t *observation,
    uint64_t now_ms, uint64_t max_age_ms);

#ifdef __cplusplus
}
#endif

#endif /* RLVGL_APP_H */
