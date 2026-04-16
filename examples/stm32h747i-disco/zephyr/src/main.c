/*
 * SPDX-License-Identifier: MIT
 *
 * Zephyr C application shell for the rlvgl STM32H747I-DISCO demo.
 *
 * Defines kernel objects (semaphores, thread stacks) and ISR wrappers,
 * then calls into the Rust static library via rlvgl_init().
 */

#include <zephyr/kernel.h>
#include <zephyr/device.h>
#include <zephyr/irq.h>
#include <zephyr/drivers/display.h>
#include <zephyr/cache.h>
#include <zephyr/input/input.h>
#include <string.h>
#include <zephyr/fs/fs.h>
#include <ff.h>

/* ── Kernel objects ──────────────────────────────────────────────────── */

K_SEM_DEFINE(erif_sem, 0, 1);
K_SEM_DEFINE(dma2d_done_sem, 0, 1);

/* ── Display info passed to Rust ─────────────────────────────────────── */

struct rlvgl_display_info {
	uint8_t *fb_front;       /* front framebuffer (currently displayed) */
	uint8_t *fb_back;        /* back framebuffer (render target) */
	uint32_t fb_len;         /* bytes per framebuffer */
	uint16_t width;          /* portrait width (480) */
	uint16_t height;         /* portrait height (800) */
	uint16_t pixel_size;     /* bytes per pixel (4 for ARGB8888) */
};

/* ── Rust FFI declarations ───────────────────────────────────────────── */

extern void rlvgl_init(struct k_sem *erif_sem,
		       struct k_sem *dma2d_done_sem,
		       const struct rlvgl_display_info *display_info);
extern void rlvgl_dsi_isr(void);
extern void rlvgl_dma2d_isr(void);

/* ── FFI wrappers for Zephyr kernel API ─────────────────────────────── */

int rlvgl_k_sem_take(struct k_sem *sem, k_timeout_t timeout)
{
	return k_sem_take(sem, timeout);
}

void rlvgl_k_sem_give(struct k_sem *sem)
{
	k_sem_give(sem);
}

/* D-cache clean — flush all dirty lines to SDRAM. */
void rlvgl_dcache_clean(void)
{
	SCB_CleanDCache();
}

/* Sleep wrapper for Rust render loop frame pacing. */
void rlvgl_k_sleep_ms(uint32_t ms)
{
	k_sleep(K_MSEC(ms));
}

/* Present: trigger Zephyr LTDC driver's double-buffer swap.
 * Writes the full back buffer as a new frame, which triggers
 * LINE ISR -> CFBAR update -> sem give. Blocks until swap completes.
 *
 * When Zephyr display is disabled (adapted_cmd path), Rust calls
 * `dsi_cmd_mode::present()` directly instead of this function. */
#if DT_HAS_CHOSEN(zephyr_display)
static const struct device *g_disp;

int rlvgl_present(const uint8_t *back_buf, uint16_t width, uint16_t height)
{
	if (!g_disp) return -1;
	struct display_buffer_descriptor desc = {
		.buf_size = width * height * 4, /* ARGB8888 */
		.width = width,
		.height = height,
		.pitch = width,
	};
	return display_write(g_disp, 0, 0, &desc, back_buf);
}
#else
int rlvgl_present(const uint8_t *back_buf, uint16_t width, uint16_t height)
{
	(void)back_buf; (void)width; (void)height;
	return -1; /* Rust uses dsi_cmd_mode::present() directly */
}
#endif

/* ── Touch input ─────────────────────────────────────────────────────── */

/* Raw USART1 banner — bypasses Zephyr console driver so we can tell
 * whether main() is running at all, independent of console routing. */
static void usart1_raw_str(const char *s)
{
	volatile uint32_t *isr = (volatile uint32_t *)0x4001101C;
	volatile uint32_t *tdr = (volatile uint32_t *)0x40011028;
	while (*s) {
		uint32_t t = 100000;
		while (!(*isr & (1 << 7))) { if (--t == 0) break; }
		*tdr = (uint32_t)(uint8_t)*s;
		s++;
	}
}

/* Touch event sent to Rust. Coordinates are raw panel space. */
struct rlvgl_touch_event {
	int16_t x;
	int16_t y;
	uint8_t pressed; /* 1=down/move, 0=up */
};

/* Rust FFI: queue a touch or key event. */
extern void rlvgl_touch_event(const struct rlvgl_touch_event *evt);
extern void rlvgl_key_event(uint16_t code, uint8_t pressed);

static int16_t touch_x, touch_y;
static bool touch_pressed;

static void input_cb(struct input_event *evt, void *user_data)
{
	ARG_UNUSED(user_data);

	/* Debug: mark every input event with 'I' on USART1 so we can tell
	 * whether Zephyr's input subsystem delivers events from FT5336 /
	 * gpio_keys at all. */
	usart1_raw_str("I");

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
	/* Joystick / GPIO keys: forward immediately */
	case INPUT_KEY_ENTER:
	case INPUT_KEY_UP:
	case INPUT_KEY_DOWN:
	case INPUT_KEY_LEFT:
	case INPUT_KEY_RIGHT:
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

/* ── Filesystem helpers for Rust FFI ──────────────────────────────────── */

/* Directory entry passed to Rust callback. */
struct rlvgl_dirent {
	char name[256];
	uint8_t is_dir; /* 1 = directory, 0 = file */
	uint32_t size;
};

/* List directory contents. Calls `cb` for each entry. Returns 0 on
 * success, negative errno on failure. */
int rlvgl_readdir(const char *path,
		  void (*cb)(const struct rlvgl_dirent *entry, void *ctx),
		  void *ctx)
{
	struct fs_dir_t dir;
	struct fs_dirent ent;
	int ret;

	fs_dir_t_init(&dir);
	ret = fs_opendir(&dir, path);
	if (ret < 0) {
		return ret;
	}

	while (1) {
		ret = fs_readdir(&dir, &ent);
		if (ret < 0 || ent.name[0] == 0) {
			break;
		}
		struct rlvgl_dirent re;
		/* Copy name, ensure NUL termination */
		strncpy(re.name, ent.name, sizeof(re.name) - 1);
		re.name[sizeof(re.name) - 1] = '\0';
		re.is_dir = (ent.type == FS_DIR_ENTRY_DIR) ? 1 : 0;
		re.size = (uint32_t)ent.size;
		cb(&re, ctx);
	}

	fs_closedir(&dir);
	return 0;
}

/* ── ISR wrappers ────────────────────────────────────────────────────── */

static void dsi_isr_wrapper(const void *arg)
{
	ARG_UNUSED(arg);
	rlvgl_dsi_isr();
}

static void dma2d_isr_wrapper(const void *arg)
{
	ARG_UNUSED(arg);
	rlvgl_dma2d_isr();
}

/* ── Main ────────────────────────────────────────────────────────────── */

int main(void)
{
	/* SRAM3 breadcrumbs: probe-rs can read these without the board
	 * being able to print to UART. Lets us tell whether main()
	 * runs at all when serial is silent. */
	*(volatile uint32_t *)0x38000200 = 0xB0070001; /* main entered */
	usart1_raw_str("\r\n[C-MAIN]\r\n");
	printk("rlvgl-zephyr: starting\n");
	usart1_raw_str("[C-after-printk]\r\n");
	*(volatile uint32_t *)0x38000200 = 0xB0070002; /* after first printk */

	/* Probe FT5336 device readiness — helps tell whether the i2c touch
	 * controller actually bound in this build (adapted_cmd overlay
	 * disables zephyr display nodes, which may have collaterally removed
	 * parents the FT5336 node depended on). */
#if DT_NODE_EXISTS(DT_NODELABEL(ft5336))
	{
		const struct device *ft5336_dev =
			DEVICE_DT_GET(DT_NODELABEL(ft5336));
		if (ft5336_dev == NULL) {
			printk("rlvgl-zephyr: ft5336 DEVICE_DT_GET returned NULL\n");
		} else if (!device_is_ready(ft5336_dev)) {
			printk("rlvgl-zephyr: ft5336 NOT READY\n");
		} else {
			printk("rlvgl-zephyr: ft5336 OK, name=%s\n",
			       ft5336_dev->name);
		}
	}
#else
	printk("rlvgl-zephyr: ft5336 DT node MISSING\n");
#endif

	/* Register DMA2D ISR (IRQ 90, pri 3). */
	irq_connect_dynamic(90, 3, dma2d_isr_wrapper, NULL, 0);
	irq_enable(90);

	/* Register DSI ISR (IRQ 123, pri 2).
	 * In video mode this is a no-op (ERIF never fires).
	 * In adapted command mode (Rust feature "adapted_cmd"), the ERIF
	 * handler clears LTDCEN after each scan, giving DMA2D exclusive
	 * SDRAM access for the render window. */
	irq_connect_dynamic(123, 2, dsi_isr_wrapper, NULL, 0);
	irq_enable(123);

	uint8_t *fb_front;
	uint8_t *fb_back;
	uint16_t fb_w;
	uint16_t fb_h;
	uint16_t px_sz = 4; /* ARGB8888 */
	uint32_t fb_len;

#if DT_HAS_CHOSEN(zephyr_display)
	/* ── Display bringup via Zephyr display API (video mode) ────── */
	g_disp = DEVICE_DT_GET(DT_CHOSEN(zephyr_display));
	if (!device_is_ready(g_disp)) {
		printk("rlvgl-zephyr: display not ready!\n");
		return -1;
	}

	struct display_capabilities caps;
	display_get_capabilities(g_disp, &caps);
	printk("rlvgl-zephyr: display %ux%u fmt=%u\n",
	       caps.x_resolution, caps.y_resolution,
	       caps.current_pixel_format);

	display_blanking_off(g_disp);
	printk("rlvgl-zephyr: blanking off\n");

	/* In video mode with rotation=90, the LTDC scans landscape:
	 * 800 pixels per line, 480 lines. */
	fb_front = (uint8_t *)display_get_framebuffer(g_disp);
	fb_w = caps.x_resolution; /* 800 */
	fb_h = caps.y_resolution; /* 480 */
	fb_len = fb_w * fb_h * px_sz;
	fb_back = fb_front + fb_len;
#else
	/* ── Adapted command mode: hardcoded FB addresses ──────────── */
	/* SDRAM Bank 0 = 0xD000_0000, Bank 1 = 0xD080_0000 (matches
	 * bare-metal). NT35510 native portrait 480x800. */
	fb_front = (uint8_t *)0xD0000000;
	fb_back  = (uint8_t *)0xD0800000;
	fb_w = 480;
	fb_h = 800;
	fb_len = fb_w * fb_h * px_sz;
	printk("rlvgl-zephyr: adapted_cmd mode (full Rust DSI init)\n");
#endif

	printk("rlvgl-zephyr: fb_front=%p fb_back=%p fb_len=%u\n",
	       fb_front, fb_back, fb_len);

	struct rlvgl_display_info di = {
		.fb_front = fb_front,
		.fb_back = fb_back,
		.fb_len = fb_len,
		.width = fb_w,
		.height = fb_h,
		.pixel_size = px_sz,
	};

	/* ── Filesystem: mount SD card ────────────────────────────── */
	static FATFS sd_fatfs;
	static struct fs_mount_t sd_mnt = {
		.type = FS_FATFS,
		.fs_data = &sd_fatfs,
		.mnt_point = "/SD:",
	};
	int fs_ret = fs_mount(&sd_mnt);
	if (fs_ret == 0) {
		printk("rlvgl-zephyr: SD mounted at /SD:\n");
	} else {
		printk("rlvgl-zephyr: SD mount failed (%d)\n", fs_ret);
	}

	*(volatile uint32_t *)0x38000200 = 0xB0070010; /* about to call rlvgl_init */
	printk("rlvgl-zephyr: calling rlvgl_init\n");
	rlvgl_init(&erif_sem, &dma2d_done_sem, &di);
	*(volatile uint32_t *)0x38000200 = 0xB0070011; /* rlvgl_init returned */
	printk("rlvgl-zephyr: init complete\n");

	/* Keep main thread alive. */
	while (1) {
		k_sleep(K_SECONDS(5));
		printk("rlvgl-zephyr: heartbeat\n");
	}

	return 0;
}
