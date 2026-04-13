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

/* Present: trigger Zephyr LTDC driver's double-buffer swap.
 * Writes the full back buffer as a new frame, which triggers
 * LINE ISR -> CFBAR update -> sem give. Blocks until swap completes. */
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
	printk("rlvgl-zephyr: starting\n");

	/* Register DMA2D ISR (IRQ 90, pri 3).
	 * DSI ISR is NOT registered in video mode — the ERIF handler
	 * clears LTDCEN which kills continuous scanning. Enable it
	 * only after switching to adapted command mode. */
	irq_connect_dynamic(90, 3, dma2d_isr_wrapper, NULL, 0);
	irq_enable(90);

	/* ── Display bringup via Zephyr display API ──────────────── */
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

	/* Get framebuffer info from Zephyr's LTDC driver.
	 *
	 * In video mode with rotation=90, the LTDC scans landscape:
	 * 800 pixels per line, 480 lines. The panel MADCTL handles rotation.
	 * Use the reported (post-rotation) dimensions as the FB layout. */
	uint8_t *fb_front = (uint8_t *)display_get_framebuffer(g_disp);
	uint16_t fb_w = caps.x_resolution; /* 800 (landscape) */
	uint16_t fb_h = caps.y_resolution; /* 480 (landscape) */
	uint16_t px_sz = 4;    /* ARGB8888 */
	uint32_t fb_len = fb_w * fb_h * px_sz;
	uint8_t *fb_back = fb_front + fb_len;

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

	printk("rlvgl-zephyr: calling rlvgl_init\n");
	rlvgl_init(&erif_sem, &dma2d_done_sem, &di);
	printk("rlvgl-zephyr: init complete\n");

	/* Keep main thread alive. */
	while (1) {
		k_sleep(K_SECONDS(5));
		printk("rlvgl-zephyr: heartbeat\n");
	}

	return 0;
}
