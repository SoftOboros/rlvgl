/*
 * FFI shims between Rust and FreeRTOS.
 *
 * Rust calls these from ISR contexts (DSI / DMA2D).  The ISR-variant
 * FreeRTOS APIs require a BaseType_t "higher priority task woken" flag
 * plus a trailing portYIELD_FROM_ISR() — packing that ceremony into a
 * single C entry point keeps the Rust ISR handlers free of churn.
 */

#include "FreeRTOS.h"
#include "semphr.h"
#include "task.h"

/* ── Cortex-M exception trampolines ─────────────────────────────────
 *
 * FreeRTOS's SVC and PendSV handlers are `__attribute__((naked))` and
 * hand-roll the Cortex-M context save. They must sit at the vector
 * slot unmodified — any prologue would corrupt the exception frame.
 *
 * cortex-m-rt emits the vector table with `SVCall`, `PendSV`, and
 * `SysTick` declared as weak defaults. Providing strong globals with
 * the same names makes the linker pick ours instead.
 *
 * `__attribute__((alias))` requires the target to be defined in the
 * same translation unit — it isn't, so we use naked tail-branch
 * trampolines instead. An unconditional `b` does not touch SP/LR, so
 * the hardware-stacked frame reaches the real handler untouched.
 */
/* D3 SRAM breadcrumbs to detect pre-scheduler invocations.
 * 0x3800_0600 = SVCall hit count, 0x3800_0604 = PendSV hit count.
 * r0/r1 are hardware-saved on the exception stack — safe to clobber
 * before branching to the real handler. No push/pop needed. */
__attribute__( ( naked ) ) void SVCall( void )
{
    __asm__ volatile (
        "ldr  r0, =0x38000600   \n"
        "ldr  r1, [r0]          \n"
        "adds r1, #1            \n"
        "str  r1, [r0]          \n"
        "b    vPortSVCHandler   \n"
    );
}

__attribute__( ( naked ) ) void PendSV( void )
{
    __asm__ volatile (
        "ldr  r0, =0x38000604   \n"
        "ldr  r1, [r0]          \n"
        "adds r1, #1            \n"
        "str  r1, [r0]          \n"
        "b    xPortPendSVHandler\n"
    );
}

void rlvgl_sem_give_from_isr( SemaphoreHandle_t sem )
{
    BaseType_t xHigherPriorityTaskWoken = pdFALSE;
    xSemaphoreGiveFromISR( sem, &xHigherPriorityTaskWoken );
    portYIELD_FROM_ISR( xHigherPriorityTaskWoken );
}

/*
 * Optional helper: give a semaphore from a task context.  Wraps
 * xSemaphoreGive so Rust doesn't need to import semphr.h macro
 * expansions directly (xSemaphoreGive is a xQueueGenericSend macro).
 */
BaseType_t rlvgl_sem_give( SemaphoreHandle_t sem )
{
    return xSemaphoreGive( sem );
}

/*
 * Wrapper around xSemaphoreTake.  xSemaphoreTake is a macro expanding
 * to xQueueSemaphoreTake, so Rust can't `extern` it directly.
 */
BaseType_t rlvgl_sem_take( SemaphoreHandle_t sem, TickType_t ticks )
{
    return xSemaphoreTake( sem, ticks );
}

/*
 * Create a binary semaphore using caller-provided static storage.
 * Rust passes a pointer to a zeroed StaticSemaphore_t; we return the
 * SemaphoreHandle_t (opaque pointer).
 */
SemaphoreHandle_t rlvgl_sem_create_binary_static( StaticSemaphore_t * buf )
{
    return xSemaphoreCreateBinaryStatic( buf );
}
