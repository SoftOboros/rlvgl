/*
 * FreeRTOS config for rlvgl STM32H747I-DISCO port.
 *
 * Static allocation only — Rust uses embedded_alloc for its heap, so we
 * don't want FreeRTOS competing for the same allocator. Tasks and
 * semaphores come from statically-declared storage on the Rust side.
 *
 * Syscall priority set at 5 (= 0x50 in NVIC.PRIORITY with 4-bit subgroup).
 * DSI/DMA2D ISRs run at priority 6-7 so they remain ISR-safe for
 * xSemaphoreGiveFromISR without being maskable by FreeRTOS critical
 * sections.
 */

#ifndef FREERTOS_CONFIG_H
#define FREERTOS_CONFIG_H

/* ── Cortex-M7 core + tick ─────────────────────────────────────────── */
#define configCPU_CLOCK_HZ                      ( 400000000UL )
/* configSYSTICK_CLOCK_HZ intentionally undefined: port.c only selects
 * the processor clock (CLK_BIT=1 in SYST_CSR) when this symbol is NOT
 * defined. Defining it — even to the same value as configCPU_CLOCK_HZ
 * — flips the port into "use reference clock" mode (HCLK/8 = 50 MHz
 * on STM32H7), which makes the effective tick rate 125 Hz instead of
 * 1 kHz and slows every vTaskDelay by 8×. */
#define configTICK_RATE_HZ                      ( 1000 )

/* ── Scheduler ─────────────────────────────────────────────────────── */
#define configUSE_PREEMPTION                    1
#define configUSE_TIME_SLICING                  1
#define configUSE_PORT_OPTIMISED_TASK_SELECTION 1
#define configUSE_TICKLESS_IDLE                 0
#define configMAX_PRIORITIES                    8
#define configMINIMAL_STACK_SIZE                ( ( unsigned short ) 128 )
#define configMAX_TASK_NAME_LEN                 ( 12 )
#define configUSE_16_BIT_TICKS                  0
#define configIDLE_SHOULD_YIELD                 1

/* ── Static allocation (preferred) ─────────────────────────────────── */
#define configSUPPORT_STATIC_ALLOCATION         1
#define configSUPPORT_DYNAMIC_ALLOCATION        1
#define configTOTAL_HEAP_SIZE                   ( ( size_t ) ( 16 * 1024 ) )
#define configAPPLICATION_ALLOCATED_HEAP        0

/* ── Synchronization primitives ────────────────────────────────────── */
#define configUSE_MUTEXES                       1
#define configUSE_RECURSIVE_MUTEXES             0
#define configUSE_COUNTING_SEMAPHORES           1
#define configQUEUE_REGISTRY_SIZE               0
#define configUSE_QUEUE_SETS                    0

/* ── Software timers ───────────────────────────────────────────────── */
#define configUSE_TIMERS                        1
#define configTIMER_TASK_PRIORITY               ( configMAX_PRIORITIES - 1 )
#define configTIMER_QUEUE_LENGTH                8
#define configTIMER_TASK_STACK_DEPTH            256

/* ── Hooks ─────────────────────────────────────────────────────────── */
#define configUSE_IDLE_HOOK                     0
#define configUSE_TICK_HOOK                     0
#define configUSE_MALLOC_FAILED_HOOK            0
#define configCHECK_FOR_STACK_OVERFLOW          0
#define configUSE_DAEMON_TASK_STARTUP_HOOK      0
#define configUSE_SB_COMPLETED_CALLBACK         0

/* ── Stats / diagnostics (off for release; enable for tuning) ─────── */
#define configGENERATE_RUN_TIME_STATS           0
#define configUSE_TRACE_FACILITY                0
#define configUSE_STATS_FORMATTING_FUNCTIONS    0

/* ── Cortex-M NVIC config ──────────────────────────────────────────── */
/* STM32H7 NVIC uses 4 priority bits (PRIO_BITS = 4).                  */
#define configPRIO_BITS                         4

/* Lowest interrupt priority that can be used by the kernel. Tick must
 * run at the lowest possible priority so long ISRs can interrupt it. */
#define configKERNEL_INTERRUPT_PRIORITY                         ( 0xF << ( 8 - configPRIO_BITS ) )

/* Highest interrupt priority from which FreeRTOS API calls can be made.
 * ISRs at numerically LOWER priorities (= higher logical priority) than
 * this cannot call any FreeRTOS API — they must be fully independent.
 * Set to 5: our display ISRs (DSI/DMA2D) run at 6-7, safely below this. */
#define configMAX_SYSCALL_INTERRUPT_PRIORITY                    ( 5 << ( 8 - configPRIO_BITS ) )
#define configLIBRARY_LOWEST_INTERRUPT_PRIORITY                 0xF
#define configLIBRARY_MAX_SYSCALL_INTERRUPT_PRIORITY            5

/* Skip vector-table verification — we route SVC/PendSV/SysTick through
 * cortex-m-rt exception handlers (see freertos_entry.rs), not directly
 * from the FreeRTOS-expected vector table. */
#define configCHECK_HANDLER_INSTALLATION        0

/* ── Optional API set ──────────────────────────────────────────────── */
#define INCLUDE_vTaskPrioritySet                1
#define INCLUDE_uxTaskPriorityGet               1
#define INCLUDE_vTaskDelete                     0
#define INCLUDE_vTaskSuspend                    1
#define INCLUDE_vTaskDelayUntil                 1
#define INCLUDE_vTaskDelay                      1
#define INCLUDE_xTaskGetSchedulerState          1
#define INCLUDE_xTaskGetCurrentTaskHandle       1
#define INCLUDE_uxTaskGetStackHighWaterMark     0
#define INCLUDE_xTaskGetIdleTaskHandle          0
#define INCLUDE_eTaskGetState                   0
#define INCLUDE_xEventGroupSetBitFromISR        1
#define INCLUDE_xTimerPendFunctionCall          1
#define INCLUDE_xSemaphoreGetMutexHolder        0

/* ── Assert ────────────────────────────────────────────────────────── */
#define configASSERT( x ) do { if ( ( x ) == 0 ) { __asm volatile ( "bkpt #0" ); for( ;; ); } } while( 0 )

#endif /* FREERTOS_CONFIG_H */
