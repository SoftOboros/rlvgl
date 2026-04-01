/* Minimal STM32H747 register definitions for C HAL bindings.
 * SPDX-FileCopyrightText: 2025 Softoboros Technology, Inc.
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * Addresses from RM0399 (STM32H747 Reference Manual).
 * Only registers used by the BSP peripheral init are defined here.
 */

#ifndef STM32H747XI_H
#define STM32H747XI_H

#include <stdint.h>

/* Volatile 32-bit register access */
#define __IO volatile
#define REG32(addr) (*((__IO uint32_t *)(addr)))

/* ---------- GPIO port base addresses (AHB4, 0x5802_0000 + 0x400*n) ------- */
#define GPIOA_BASE  0x58020000UL
#define GPIOB_BASE  0x58020400UL
#define GPIOC_BASE  0x58020800UL
#define GPIOD_BASE  0x58020C00UL
#define GPIOE_BASE  0x58021000UL
#define GPIOF_BASE  0x58021400UL
#define GPIOG_BASE  0x58021800UL
#define GPIOH_BASE  0x58021C00UL
#define GPIOI_BASE  0x58022000UL
#define GPIOJ_BASE  0x58022400UL
#define GPIOK_BASE  0x58022800UL

/* ---------- GPIO register offsets ---------------------------------------- */
#define GPIO_MODER   0x00
#define GPIO_OTYPER  0x04
#define GPIO_OSPEEDR 0x08
#define GPIO_PUPDR   0x0C
#define GPIO_IDR     0x10
#define GPIO_ODR     0x14
#define GPIO_BSRR    0x18
#define GPIO_LCKR    0x1C
#define GPIO_AFRL    0x20
#define GPIO_AFRH    0x24

/* ---------- RCC base + clock-enable register offsets --------------------- */
#define RCC_BASE     0x58024400UL
#define RCC_AHB4ENR  (RCC_BASE + 0x0E0)
#define RCC_APB1LENR (RCC_BASE + 0x0E8)
#define RCC_APB1HENR (RCC_BASE + 0x0EC)
#define RCC_APB2ENR  (RCC_BASE + 0x0F0)
#define RCC_APB4ENR  (RCC_BASE + 0x0F4)

/* ---------- NVIC (Cortex-M7 private peripheral bus) ---------------------- */
#define NVIC_ICER_BASE 0xE000E180UL  /* Interrupt Clear-Enable Registers */

/* Convenience: disable IRQ n */
static inline void nvic_disable_irq(uint32_t irq) {
    REG32(NVIC_ICER_BASE + (irq / 32) * 4) = (1u << (irq % 32));
}

/* ---------- IRQ numbers for BSP peripherals ------------------------------ */
#define IRQ_SPI2     36
#define IRQ_USART1   37
#define IRQ_UART8    83
#define IRQ_SPI5     85
#define IRQ_I2C4_EV  95
#define IRQ_I2C4_ER  96

/* ---------- GPIO register access helper ---------------------------------- */
#define GPIOx(base, off) REG32((base) + (off))

#endif /* STM32H747XI_H */
