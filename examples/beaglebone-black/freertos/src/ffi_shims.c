/*
 * FreeRTOS FFI shims for Rust — BeagleBone Black (Cortex-A8)
 *
 * FreeRTOS semaphore/task APIs are macros in C and cannot be called
 * directly from Rust FFI. These thin wrappers expose them as real
 * functions that Rust can link against.
 */

#include "FreeRTOS.h"
#include "semphr.h"
#include "task.h"

/*
 * Create a binary semaphore using caller-provided static storage.
 */
SemaphoreHandle_t rlvgl_sem_create_binary_static( StaticSemaphore_t * buf )
{
    return xSemaphoreCreateBinaryStatic( buf );
}

/*
 * Take (wait on) a semaphore from task context.
 */
BaseType_t rlvgl_sem_take( SemaphoreHandle_t sem, TickType_t ticks )
{
    return xSemaphoreTake( sem, ticks );
}

/*
 * Give (signal) a semaphore from task context.
 */
BaseType_t rlvgl_sem_give( SemaphoreHandle_t sem )
{
    return xSemaphoreGive( sem );
}

/*
 * Give a semaphore from ISR context.
 * Handles portYIELD_FROM_ISR internally.
 */
void rlvgl_sem_give_from_isr( SemaphoreHandle_t sem )
{
    BaseType_t xHigherPriorityTaskWoken = pdFALSE;
    xSemaphoreGiveFromISR( sem, &xHigherPriorityTaskWoken );
    portYIELD_FROM_ISR( xHigherPriorityTaskWoken );
}

/*
 * Static allocation callbacks required by FreeRTOS when
 * configSUPPORT_STATIC_ALLOCATION is enabled.
 */
static StaticTask_t xIdleTaskTCB;
static StackType_t  uxIdleTaskStack[ configMINIMAL_STACK_SIZE ];

void vApplicationGetIdleTaskMemory( StaticTask_t **ppxIdleTaskTCBBuffer,
                                    StackType_t **ppxIdleTaskStackBuffer,
                                    uint32_t *pulIdleTaskStackSize )
{
    *ppxIdleTaskTCBBuffer   = &xIdleTaskTCB;
    *ppxIdleTaskStackBuffer = uxIdleTaskStack;
    *pulIdleTaskStackSize   = configMINIMAL_STACK_SIZE;
}
