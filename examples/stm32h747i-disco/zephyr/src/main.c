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

/* ── Kernel objects ──────────────────────────────────────────────────── */

/* ERIF semaphore: given by DSI ISR, taken by present thread.
 * Max count 1 — additional gives are silently dropped, matching the
 * bare-metal AtomicBool behavior. */
K_SEM_DEFINE(erif_sem, 0, 1);

/* DMA2D transfer-complete semaphore: given by DMA2D ISR, taken by
 * render thread. Max count 1. */
K_SEM_DEFINE(dma2d_done_sem, 0, 1);

/* ── Rust FFI declarations ───────────────────────────────────────────── */

/* Initialization entry point — passes kernel object pointers to Rust. */
extern void rlvgl_init(struct k_sem *erif_sem, struct k_sem *dma2d_done_sem);

/* ISR handlers implemented in Rust (zephyr_entry.rs). */
extern void rlvgl_dsi_isr(void);
extern void rlvgl_dma2d_isr(void);

/* ── FFI wrappers for Rust ───────────────────────────────────────────── */

/* Zephyr k_sem_give/k_sem_take may be static-inline or macro-expanded.
 * Provide real symbols so the Rust static library can link against them. */
int rlvgl_k_sem_take(struct k_sem *sem, k_timeout_t timeout)
{
	return k_sem_take(sem, timeout);
}

void rlvgl_k_sem_give(struct k_sem *sem)
{
	k_sem_give(sem);
}

/* ── ISR wrappers ────────────────────────────────────────────────────── */

/* DSI IRQ 123 on STM32H747. Priority 1 (high — timing-critical). */
static void dsi_isr_wrapper(const void *arg)
{
	ARG_UNUSED(arg);
	rlvgl_dsi_isr();
}

/* DMA2D IRQ. Priority 3 (below DSI, above normal threads). */
static void dma2d_isr_wrapper(const void *arg)
{
	ARG_UNUSED(arg);
	rlvgl_dma2d_isr();
}

/* ── Main ────────────────────────────────────────────────────────────── */

int main(void)
{
	printk("rlvgl-zephyr: starting\n");
	/* Register ISRs dynamically (CONFIG_DYNAMIC_INTERRUPTS=y).
	 *
	 * IRQ numbers from stm32h747xx.h:
	 *   DSI_IRQn   = 78  (NVIC IRQ, not exception number)
	 *   DMA2D_IRQn = 90
	 *
	 * Note: Zephyr IRQ numbers are NVIC IRQ numbers (0-based),
	 * not Cortex-M exception numbers (which are IRQ+16).
	 */
	irq_connect_dynamic(78, 1, dsi_isr_wrapper, NULL, 0);
	irq_enable(78);

	irq_connect_dynamic(90, 3, dma2d_isr_wrapper, NULL, 0);
	irq_enable(90);

	/* Hand off to Rust. This initializes the display subsystem and
	 * (in the future) spawns render/present/touch threads. */
	printk("rlvgl-zephyr: calling rlvgl_init\n");
	rlvgl_init(&erif_sem, &dma2d_done_sem);
	printk("rlvgl-zephyr: init complete\n");

	/* Keep main thread alive — Zephyr idle thread handles power mgmt. */
	while (1) {
		k_sleep(K_SECONDS(1));
		printk("rlvgl-zephyr: heartbeat\n");
	}

	return 0;
}
