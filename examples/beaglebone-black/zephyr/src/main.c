/*
 * SPDX-License-Identifier: MIT
 *
 * Zephyr main() for BeagleBone Black + NHD-7.0CTP-CAPE-P rlvgl demo.
 *
 * Responsibilities:
 * - Allocate framebuffers in DDR
 * - Register input callbacks (touch, keys)
 * - Call Rust rlvgl_init() with kernel objects and display info
 * - Provide FFI wrappers for Zephyr kernel API (k_sem, k_sleep, etc.)
 */

#include <zephyr/kernel.h>
#include <zephyr/device.h>
#include <zephyr/drivers/display.h>
#include <zephyr/input/input.h>
#include <zephyr/drivers/gpio.h>

#include <string.h>
#include <stdint.h>

/* ---- Display configuration ---- */

#define FB_WIDTH   800
#define FB_HEIGHT  480
#define FB_BPP     4   /* ARGB8888 */
#define FB_SIZE    (FB_WIDTH * FB_HEIGHT * FB_BPP)

/* Framebuffers in DDR (statically allocated) */
static uint8_t fb_front[FB_SIZE] __attribute__((aligned(64)));
static uint8_t fb_back[FB_SIZE]  __attribute__((aligned(64)));

/* ---- Display info struct (passed to Rust) ---- */

struct rlvgl_display_info {
    uint8_t  *fb_front;
    uint8_t  *fb_back;
    uint32_t  fb_len;
    uint16_t  width;
    uint16_t  height;
    uint16_t  pixel_size;
};

/* ---- Kernel semaphores ---- */

K_SEM_DEFINE(eof_sem, 0, 1);      /* LCDC EOF ISR gives this */

/* ---- Extern Rust functions ---- */

extern void rlvgl_init(struct k_sem *eof_sem,
                       const struct rlvgl_display_info *display_info);
extern void rlvgl_touch_event(const struct rlvgl_touch_event *evt);
extern void rlvgl_key_event(uint16_t code, uint8_t pressed);
extern void rlvgl_lcdc_eof_isr(void);

/* ---- Touch event struct (matches Rust repr(C)) ---- */

struct rlvgl_touch_event {
    int16_t x;
    int16_t y;
    uint8_t pressed;
};

/* ---- Input callback ---- */

static int16_t touch_x, touch_y;
static bool touch_pressed;

static void input_cb(struct input_event *evt, void *user_data)
{
    ARG_UNUSED(user_data);

    switch (evt->code) {
    case INPUT_ABS_X:
        touch_x = (int16_t)evt->value;
        break;
    case INPUT_ABS_Y:
        touch_y = (int16_t)evt->value;
        break;
    case INPUT_BTN_TOUCH:
        touch_pressed = evt->value != 0;
        break;
    default:
        /* Forward key events immediately */
        rlvgl_key_event(evt->code, evt->value ? 1 : 0);
        return;
    }

    if (evt->sync) {
        struct rlvgl_touch_event te = {
            .x = touch_x,
            .y = touch_y,
            .pressed = touch_pressed ? 1 : 0,
        };
        rlvgl_touch_event(&te);
    }
}
INPUT_CALLBACK_DEFINE(NULL, input_cb, NULL);

/* ---- FFI wrappers for Zephyr kernel API ---- */

int rlvgl_k_sem_take(struct k_sem *sem, int32_t timeout_ms)
{
    k_timeout_t t = (timeout_ms < 0) ? K_FOREVER : K_MSEC(timeout_ms);
    return k_sem_take(sem, t);
}

void rlvgl_k_sem_give(struct k_sem *sem)
{
    k_sem_give(sem);
}

void rlvgl_k_sleep_ms(uint32_t ms)
{
    k_sleep(K_MSEC(ms));
}

/* ---- LCDC EOF ISR wrapper ---- */

static void lcdc_eof_isr_wrapper(const void *arg)
{
    ARG_UNUSED(arg);
    rlvgl_lcdc_eof_isr();
    k_sem_give(&eof_sem);
}

/* ---- Main ---- */

int main(void)
{
    /* Clear framebuffers */
    memset(fb_front, 0, FB_SIZE);
    memset(fb_back, 0, FB_SIZE);

    /* Register LCDC EOF interrupt (IRQ 36 on AM335x) */
    irq_connect_dynamic(36, 3, lcdc_eof_isr_wrapper, NULL, 0);
    irq_enable(36);

    /* Prepare display info */
    struct rlvgl_display_info info = {
        .fb_front   = fb_front,
        .fb_back    = fb_back,
        .fb_len     = FB_SIZE,
        .width      = FB_WIDTH,
        .height     = FB_HEIGHT,
        .pixel_size = FB_BPP,
    };

    /* Hand off to Rust — this may not return if Rust runs its own loop */
    rlvgl_init(&eof_sem, &info);

    /* If rlvgl_init returns, enter idle */
    while (1) {
        k_sleep(K_FOREVER);
    }

    return 0;
}
