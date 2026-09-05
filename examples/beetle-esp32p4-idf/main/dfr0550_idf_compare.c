/*
 * dfr0550_idf_compare.c - ESP-IDF control path for the DFR0550-V2 MIPI-DSI panel.
 */

#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "driver/i2c_master.h"
#include "esp_cache.h"
#include "esp_err.h"
#include "esp_lcd_mipi_dsi.h"
#include "esp_lcd_panel_ops.h"
#include "esp_ldo_regulator.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "hal/gpio_types.h"
#include "soc/mipi_dsi_host_reg.h"
#include "soc/soc.h"

#include "rlvgl_app.h"

static const char *TAG = "dfr0550_idf";

enum {
    DFR0550_H_RES = 800,
    DFR0550_V_RES = 480,
    DFR0550_HSYNC = 2,
    DFR0550_HBP = 46,
    DFR0550_HFP = 1,
    DFR0550_VSYNC = 2,
    DFR0550_VBP = 21,
    DFR0550_VFP = 7,
    DFR0550_DPI_CLK_MHZ = 26,
    DFR0550_DSI_LANES = 1,
    DFR0550_DSI_LANE_MBPS = 750,
    DFR0550_FB_BYTES = DFR0550_H_RES * DFR0550_V_RES * 3,
};

enum {
    DFR0550_I2C_ADDR = 0x45,
    DFR0550_REG_PORTA = 0x81,
    DFR0550_REG_PORTB = 0x82,
    DFR0550_REG_POWERON = 0x85,
    DFR0550_REG_PWM = 0x86,
    DFR0550_PORTA_KERNEL_DEFAULT = 0x04,
};

enum {
    /*
     * FocalTech FT5x06/FT6x36 capacitive touch, on the SAME FFC I2C bus as the
     * 0x45 bridge (SDA=GPIO7, SCL=GPIO8). 5-point; we read point 1 only here.
     * Register map: 0x02 = TD_STATUS (low nibble = #points); each point is six
     * bytes from 0x03: XH[evt:2|.. |x_hi:4], XL, YH[id:4|y_hi:4], YL, weight,
     * misc. Event flag (XH>>6): 0=down 1=up 2=contact 3=no-event.
     */
    FT5X06_I2C_ADDR = 0x38,
    FT5X06_REG_TD_STATUS = 0x02,
};

static esp_ldo_channel_handle_t s_dphy_ldo;
static i2c_master_bus_handle_t s_i2c_bus;
static i2c_master_dev_handle_t s_bridge;

static esp_err_t bridge_write(i2c_master_dev_handle_t bridge, uint8_t reg, uint8_t value)
{
    uint8_t data[] = {reg, value};
    return i2c_master_transmit(bridge, data, sizeof(data), 1000);
}

static esp_err_t bridge_read(i2c_master_dev_handle_t bridge, uint8_t reg, uint8_t *value)
{
    *value = 0;
    return i2c_master_transmit_receive(bridge, &reg, 1, value, 1, 1000);
}

static esp_err_t bridge_i2c_init(i2c_master_dev_handle_t *bridge_out)
{
    *bridge_out = NULL;
    i2c_master_bus_handle_t bus = NULL;
    i2c_master_bus_config_t bus_config = {
        .i2c_port = I2C_NUM_0,
        .sda_io_num = GPIO_NUM_7,
        .scl_io_num = GPIO_NUM_8,
        .clk_source = I2C_CLK_SRC_DEFAULT,
        .glitch_ignore_cnt = 7,
        .trans_queue_depth = 0,
        .flags = {
            .enable_internal_pullup = true,
        },
    };
    esp_err_t err = i2c_new_master_bus(&bus_config, &bus);
    if (err != ESP_OK) {
        return err;
    }
    s_i2c_bus = bus;

    /* A missing DFR0550 is an allowed CCPS operating state. Probe before any
     * bridge writes so the battery poller can continue without a display. */
    err = i2c_master_probe(bus, DFR0550_I2C_ADDR, 100);
    if (err != ESP_OK) {
        return err;
    }

    i2c_master_dev_handle_t bridge = NULL;
    i2c_device_config_t device_config = {
        .dev_addr_length = I2C_ADDR_BIT_LEN_7,
        .device_address = DFR0550_I2C_ADDR,
        .scl_speed_hz = 100000,
        .scl_wait_us = 0,
    };
    err = i2c_master_bus_add_device(bus, &device_config, &bridge);
    if (err == ESP_OK) {
        *bridge_out = bridge;
    }
    return err;
}

static esp_err_t bridge_wake(i2c_master_dev_handle_t bridge)
{
    ESP_LOGI(TAG, "Wake DFR0550 bridge over IDF I2C");
    esp_err_t err = bridge_write(bridge, DFR0550_REG_POWERON, 1);
    if (err != ESP_OK) {
        return err;
    }
    vTaskDelay(pdMS_TO_TICKS(20));

    for (int tries = 0; tries < 100; tries++) {
        uint8_t portb = 0;
        err = bridge_read(bridge, DFR0550_REG_PORTB, &portb);
        if (err == ESP_OK && (portb & 0x01) != 0) {
            ESP_LOGI(TAG, "Bridge ready, PORTB=0x%02x", portb);
            err = bridge_write(bridge, DFR0550_REG_PORTA, DFR0550_PORTA_KERNEL_DEFAULT);
            if (err == ESP_OK) {
                err = bridge_write(bridge, DFR0550_REG_PWM, 255);
            }
            return err;
        }
        if (err != ESP_OK) {
            ESP_LOGD(TAG, "Bridge PORTB read not ready yet: %s", esp_err_to_name(err));
        }
        vTaskDelay(pdMS_TO_TICKS(10));
    }

    return ESP_ERR_TIMEOUT;
}

static i2c_master_dev_handle_t touch_i2c_init(void)
{
    esp_err_t err = i2c_master_probe(s_i2c_bus, FT5X06_I2C_ADDR, 100);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "FT5x06 touch absent; display remains usable without touch");
        return NULL;
    }

    i2c_master_dev_handle_t touch = NULL;
    i2c_device_config_t device_config = {
        .dev_addr_length = I2C_ADDR_BIT_LEN_7,
        .device_address = FT5X06_I2C_ADDR,
        .scl_speed_hz = 100000,
        .scl_wait_us = 0,
    };
    err = i2c_master_bus_add_device(s_i2c_bus, &device_config, &touch);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "FT5x06 attach failed: %s", esp_err_to_name(err));
        return NULL;
    }
    ESP_LOGI(TAG, "FT5x06 touch attached at I2C 0x%02x", FT5X06_I2C_ADDR);
    return touch;
}

/*
 * Read touch point 1 from the FT5x06. Returns true with x and y set when a
 * finger is down (event flag down/contact); false on no-touch, lift, or error.
 */
static bool touch_read(i2c_master_dev_handle_t touch, int *x, int *y)
{
    if (touch == NULL) {
        return false;
    }
    uint8_t reg = FT5X06_REG_TD_STATUS;
    uint8_t buf[5] = {0};
    esp_err_t err = i2c_master_transmit_receive(touch, &reg, 1, buf, sizeof(buf), 100);
    if (err != ESP_OK) {
        return false;
    }
    uint8_t points = buf[0] & 0x0f;
    if (points == 0 || points > 5) {
        return false;
    }
    uint8_t event = buf[1] >> 6; /* 0=down 1=up 2=contact 3=no-event */
    if (event == 1 || event == 3) {
        return false;
    }
    int raw_x = ((buf[1] & 0x0f) << 8) | buf[2];
    int raw_y = ((buf[3] & 0x0f) << 8) | buf[4];
    /*
     * The FT5x06 frame is point-reflected through center relative to the DSI
     * scanout (center hits, but motion runs opposite on both axes), i.e. the
     * panel is mounted 180 degrees from the touch panel's native origin.
     * Flip both axes into screen space (verified on hardware 2026-06-15).
     */
    *x = (DFR0550_H_RES - 1) - raw_x;
    *y = (DFR0550_V_RES - 1) - raw_y;
    return true;
}

/*
 * Host hook called by the Rust rlvgl payload when the disco controller asks
 * for a backlight change (DiscoCommand::SetBacklight). Maps the abstract
 * 0..100 level to the DFR0550 bridge's 0..255 PWM register. Non-static so the
 * Rust staticlib resolves it at link. Runs on the render task, same as every
 * other bridge I2C access, so no extra synchronization is needed.
 */
void rlvgl_host_set_backlight(uint8_t level)
{
    if (level > 100) {
        level = 100;
    }
    uint8_t pwm = (uint8_t)((uint32_t)level * 255u / 100u);
    if (s_bridge != NULL) {
        esp_err_t err = bridge_write(s_bridge, DFR0550_REG_PWM, pwm);
        if (err == ESP_OK) {
            ESP_LOGI(TAG, "Backlight %u%% -> PWM %u", level, pwm);
        } else {
            ESP_LOGW(TAG, "Backlight write failed: %s", esp_err_to_name(err));
        }
    }
}

static esp_err_t dphy_power_on(void)
{
    esp_ldo_channel_config_t ldo_config = {
        .chan_id = 3,
        .voltage_mv = 2500,
    };
    esp_err_t err = esp_ldo_acquire_channel(&ldo_config, &s_dphy_ldo);
    if (err != ESP_OK) {
        return err;
    }
    ESP_LOGI(TAG, "MIPI DSI PHY powered from LDO_VO3 at 2500 mV");
    return ESP_OK;
}

static esp_err_t panel_init(void **fb0, void **fb1, esp_lcd_panel_handle_t *panel_out)
{
    *panel_out = NULL;
    esp_lcd_dsi_bus_handle_t dsi_bus = NULL;
    esp_lcd_dsi_bus_config_t bus_config = {
        .bus_id = 0,
        .num_data_lanes = DFR0550_DSI_LANES,
        .phy_clk_src = MIPI_DSI_PHY_CLK_SRC_DEFAULT,
        .lane_bit_rate_mbps = DFR0550_DSI_LANE_MBPS,
    };
    esp_err_t err = esp_lcd_new_dsi_bus(&bus_config, &dsi_bus);
    if (err != ESP_OK) {
        return err;
    }

    uint32_t phy_status = REG_READ(DSI_HOST_PHY_STATUS_REG);
    ESP_LOGI(TAG, "MIPI_DSI_HOST.phy_status=0x%08" PRIx32, phy_status);

    esp_lcd_panel_handle_t dpi_panel = NULL;
    esp_lcd_dpi_panel_config_t dpi_config = {
        .virtual_channel = 0,
        .dpi_clk_src = MIPI_DSI_DPI_CLK_SRC_DEFAULT,
        .dpi_clock_freq_mhz = DFR0550_DPI_CLK_MHZ,
        .pixel_format = LCD_COLOR_PIXEL_FORMAT_RGB888,
        .in_color_format = LCD_COLOR_FMT_RGB888,
        .out_color_format = LCD_COLOR_FMT_RGB888,
        .num_fbs = 2,   /* double-buffered: render off-screen, flip tear-free */
        .video_timing = {
            .h_size = DFR0550_H_RES,
            .v_size = DFR0550_V_RES,
            .hsync_back_porch = DFR0550_HBP,
            .hsync_pulse_width = DFR0550_HSYNC,
            .hsync_front_porch = DFR0550_HFP,
            .vsync_back_porch = DFR0550_VBP,
            .vsync_pulse_width = DFR0550_VSYNC,
            .vsync_front_porch = DFR0550_VFP,
        },
        .flags = {
            .use_dma2d = true,
            .disable_lp = false,
        },
    };

    err = esp_lcd_new_panel_dpi(dsi_bus, &dpi_config, &dpi_panel);
    if (err != ESP_OK) {
        return err;
    }
    err = esp_lcd_panel_init(dpi_panel);
    if (err != ESP_OK) {
        return err;
    }
    err = esp_lcd_dpi_panel_get_frame_buffer(dpi_panel, 2, fb0, fb1);
    if (err != ESP_OK) {
        return err;
    }
    ESP_LOGI(TAG, "DPI framebuffers at %p / %p (%u bytes each)", *fb0, *fb1, DFR0550_FB_BYTES);
    *panel_out = dpi_panel;
    return ESP_OK;
}

#if CONFIG_DFR0550_POWER_OFF_AFTER_WAKE
static void bridge_power_off(i2c_master_dev_handle_t bridge)
{
    uint8_t portb = 0;
    ESP_LOGW(TAG, "Command DFR0550 PWM=0 and POWERON=0 over IDF I2C");
    bridge_write(bridge, DFR0550_REG_PWM, 0);
    vTaskDelay(pdMS_TO_TICKS(20));
    bridge_write(bridge, DFR0550_REG_POWERON, 0);
    vTaskDelay(pdMS_TO_TICKS(100));

    esp_err_t err = bridge_read(bridge, DFR0550_REG_PORTB, &portb);
    if (err == ESP_OK) {
        ESP_LOGI(TAG, "After power-off command, PORTB=0x%02x", portb);
    } else {
        ESP_LOGW(TAG, "PORTB read after power-off command failed: %s", esp_err_to_name(err));
    }
}
#endif

#if CONFIG_DFR0550_COLOR_CYCLE
static void fill_rgb888(void *framebuffer, uint8_t r, uint8_t g, uint8_t b)
{
    uint8_t *fb = (uint8_t *)framebuffer;
    for (size_t i = 0; i < DFR0550_FB_BYTES; i += 3) {
        fb[i + 0] = r;
        fb[i + 1] = g;
        fb[i + 2] = b;
    }
    ESP_ERROR_CHECK(esp_cache_msync(framebuffer, DFR0550_FB_BYTES,
                                    ESP_CACHE_MSYNC_FLAG_DIR_C2M | ESP_CACHE_MSYNC_FLAG_UNALIGNED));
}
#endif

#if !CONFIG_DFR0550_COLOR_CYCLE
/* BEETLE M1/M3: hand the framebuffer to the Rust rlvgl payload (with the latest
 * touch point, if any), then writeback. */
static void render_rlvgl(void *framebuffer, int touch_x, int touch_y, int touch_active)
{
    rlvgl_app_render((uint8_t *)framebuffer, DFR0550_H_RES, DFR0550_V_RES,
                     touch_x, touch_y, touch_active);
    ESP_ERROR_CHECK(esp_cache_msync(framebuffer, DFR0550_FB_BYTES,
                                    ESP_CACHE_MSYNC_FLAG_DIR_C2M | ESP_CACHE_MSYNC_FLAG_UNALIGNED));
}
#endif

static void display_offline_forever(const char *stage, esp_err_t err) __attribute__((noreturn));

static void display_offline_forever(const char *stage, esp_err_t err)
{
    ESP_LOGW(TAG, "DISPLAY OFFLINE at %s: %s; application services continue",
             stage, esp_err_to_name(err));
    for (;;) {
        vTaskDelay(pdMS_TO_TICKS(1000));
    }
}

void app_main(void)
{
    ESP_LOGI(TAG, "ESP-IDF DFR0550 comparison app starting");
    rlvgl_app_init();

#if CONFIG_DFR0550_NO_TOUCH
    display_offline_forever("compile-time no-display diagnostic", ESP_ERR_NOT_SUPPORTED);
#endif

    i2c_master_dev_handle_t bridge = NULL;
    esp_err_t err = bridge_i2c_init(&bridge);
    if (err != ESP_OK) {
        display_offline_forever("DFR0550 I2C probe", err);
    }
    err = bridge_wake(bridge);
    if (err != ESP_OK) {
        display_offline_forever("DFR0550 bridge wake", err);
    }
    s_bridge = bridge; /* expose to the Rust backlight hook (M5) */

#if CONFIG_DFR0550_POWER_OFF_AFTER_WAKE
    bridge_power_off(bridge);
    ESP_LOGW(TAG, "Power-off-after-wake mode active: DSI/DPI/framebuffer init is skipped");
    for (;;) {
        vTaskDelay(pdMS_TO_TICKS(1000));
    }
#endif

#if CONFIG_DFR0550_WAKE_ONLY
    ESP_LOGW(TAG, "Wake-only mode active: DSI/DPI/framebuffer init is skipped");
    for (;;) {
        vTaskDelay(pdMS_TO_TICKS(1000));
    }
#endif

    err = dphy_power_on();
    if (err != ESP_OK) {
        display_offline_forever("MIPI DSI PHY power", err);
    }

    void *fb[2] = {NULL, NULL};
    esp_lcd_panel_handle_t panel = NULL;
    err = panel_init(&fb[0], &fb[1], &panel);
    if (err != ESP_OK) {
        display_offline_forever("MIPI DSI/DPI initialization", err);
    }

#if CONFIG_DFR0550_COLOR_CYCLE
    const uint8_t colors[][3] = {
        {255, 0, 0},
        {0, 255, 0},
        {0, 0, 255},
        {255, 255, 255},
        {0, 0, 0},
    };
    const char *names[] = {"red", "green", "blue", "white", "black"};

    (void)panel; /* color cycle writes the active buffer directly, no flip */
    for (size_t frame = 0;; frame++) {
        size_t idx = frame % (sizeof(colors) / sizeof(colors[0]));
        ESP_LOGI(TAG, "Fill %s", names[idx]);
        fill_rgb888(fb[0], colors[idx][0], colors[idx][1], colors[idx][2]);
        vTaskDelay(pdMS_TO_TICKS(1000));
    }
#else
    /*
     * BEETLE M1: the Rust rlvgl payload owns the pixels. Double-buffered to
     * avoid tearing — each iteration renders into the off-screen buffer, then
     * esp_lcd_panel_draw_bitmap() flips it in on the panel's vertical blank.
     * The continuous flip also satisfies the bridge's need for steady refresh
     * activity (a one-shot paint desyncs it). esp_panel_init() leaves fb[0]
     * active, so render starts on fb[1].
     */
    ESP_LOGI(TAG, "Rendering rlvgl widget tree (M1, double-buffered)");
    i2c_master_dev_handle_t touch = touch_i2c_init();
    int back = 1;
    int last_x = 0, last_y = 0, last_active = 0;
    for (size_t frame = 0;; frame++) {
        int tx = 0, ty = 0;
        int active = touch_read(touch, &tx, &ty) ? 1 : 0;

        /* Serial trace for position validation: rising/falling edges always,
         * plus a throttled trace while held so drag motion is visible. */
        if (active && !last_active) {
            ESP_LOGI(TAG, "touch down  %d,%d", tx, ty);
        } else if (!active && last_active) {
            ESP_LOGI(TAG, "touch up    %d,%d", last_x, last_y);
        } else if (active && (frame % 8 == 0)) {
            ESP_LOGI(TAG, "touch move  %d,%d", tx, ty);
        }
        last_active = active;
        if (active) {
            last_x = tx;
            last_y = ty;
        }

        render_rlvgl(fb[back], tx, ty, active);
        ESP_ERROR_CHECK(esp_lcd_panel_draw_bitmap(panel, 0, 0, DFR0550_H_RES,
                                                  DFR0550_V_RES, fb[back]));
        back ^= 1;
        vTaskDelay(pdMS_TO_TICKS(33)); /* ~30 Hz refresh */
    }
#endif
}
