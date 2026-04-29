/*
 * FreeRTOS configuration for BeagleBone Black (AM3358, Cortex-A8 @ 1 GHz).
 *
 * Key differences from STM32H747I-DISCO config:
 * - No SysTick — Cortex-A8 uses DMTIMER for tick
 * - No NVIC — AM335x uses its own INTC (0x4820_0000)
 * - configPRIO_BITS not applicable (INTC has different priority model)
 * - ARM_CA8 port uses SVC/IRQ mode switching instead of SVC/PendSV
 */

#ifndef FREERTOS_CONFIG_H
#define FREERTOS_CONFIG_H

/* Scheduler */
#define configUSE_PREEMPTION                    1
#define configUSE_TIME_SLICING                  1
#define configUSE_PORT_OPTIMISED_TASK_SELECTION  0

/* CPU */
#define configCPU_CLOCK_HZ                      ( 1000000000UL )  /* 1 GHz */
#define configTICK_RATE_HZ                      ( 1000 )          /* 1 ms per tick */

/* Task */
#define configMAX_PRIORITIES                    8
#define configMINIMAL_STACK_SIZE                ( 128 )  /* words */
#define configMAX_TASK_NAME_LEN                 16
#define configIDLE_SHOULD_YIELD                 1

/* Memory */
#define configSUPPORT_STATIC_ALLOCATION         1
#define configSUPPORT_DYNAMIC_ALLOCATION        1
#define configTOTAL_HEAP_SIZE                   ( 16 * 1024 )  /* 16 KB for kernel internals */
#define configAPPLICATION_ALLOCATED_HEAP        0

/* Synchronization */
#define configUSE_MUTEXES                       1
#define configUSE_RECURSIVE_MUTEXES             0
#define configUSE_COUNTING_SEMAPHORES           1
#define configUSE_QUEUE_SETS                    0

/* Timers */
#define configUSE_TIMERS                        0
#define configTIMER_TASK_PRIORITY               2
#define configTIMER_QUEUE_LENGTH                10
#define configTIMER_TASK_STACK_DEPTH            configMINIMAL_STACK_SIZE

/* Hooks */
#define configUSE_IDLE_HOOK                     0
#define configUSE_TICK_HOOK                     0
#define configUSE_MALLOC_FAILED_HOOK            0
#define configCHECK_FOR_STACK_OVERFLOW          0  /* Enable for debugging */

/* Cortex-A specific */
/* The ARM_CA8 FreeRTOS port uses GIC or INTC for interrupt routing.
 * configINTERRUPT_CONTROLLER_* macros are defined by the port layer
 * or left to the application. On AM335x, the INTC is at 0x4820_0000. */
#define configUNIQUE_INTERRUPT_PRIORITIES       128  /* AM335x INTC supports 128 priority levels */
#define configMAX_API_CALL_INTERRUPT_PRIORITY   18   /* IRQs at priority >= 18 may call *FromISR */

/* Debug */
#define configUSE_TRACE_FACILITY                0
#define configGENERATE_RUN_TIME_STATS           0

/* Static allocation callbacks — must be provided by application */
/* (Implemented in ffi_shims.c) */

/* Include API functions */
#define INCLUDE_vTaskPrioritySet                0
#define INCLUDE_uxTaskPriorityGet               0
#define INCLUDE_vTaskDelete                     0
#define INCLUDE_vTaskSuspend                    1
#define INCLUDE_xResumeFromISR                  0
#define INCLUDE_vTaskDelayUntil                 1
#define INCLUDE_vTaskDelay                      1
#define INCLUDE_xTaskGetSchedulerState          0
#define INCLUDE_xTaskGetCurrentTaskHandle       0
#define INCLUDE_uxTaskGetStackHighWaterMark     0
#define INCLUDE_xTaskGetIdleTaskHandle          0

#endif /* FREERTOS_CONFIG_H */
