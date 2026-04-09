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

/* GPIO register access helpers */
#define GPIOx(base, off) REG32((base) + (off))

/* Set/clear individual output bits via BSRR */
#define GPIO_SET(base, pin)   REG32((base) + GPIO_BSRR) = (1u << (pin))
#define GPIO_CLEAR(base, pin) REG32((base) + GPIO_BSRR) = (1u << ((pin) + 16))

/* ---------- RCC base address and clock-enable registers ------------------- */
#define RCC_BASE        0x58024400UL

/* AHB clock enables */
#define RCC_AHB3ENR     (RCC_BASE + 0x0D4)   /* FMC, QSPI */
#define RCC_AHB1ENR     (RCC_BASE + 0x0D8)
#define RCC_AHB2ENR     (RCC_BASE + 0x0DC)
#define RCC_AHB4ENR     (RCC_BASE + 0x0E0)   /* GPIO A-K */

/* APB clock enables */
#define RCC_APB3ENR     (RCC_BASE + 0x0E4)   /* LTDC, DSI */
#define RCC_APB1LENR    (RCC_BASE + 0x0E8)
#define RCC_APB1HENR    (RCC_BASE + 0x0EC)
#define RCC_APB2ENR     (RCC_BASE + 0x0F0)   /* TIM8, USART1, SPI2/5 */
#define RCC_APB4ENR     (RCC_BASE + 0x0F4)   /* PWR, SYSCFG */

/* CM7 domain clock enables (mirror of AHBx for core 1) */
#define RCC_C1_AHB3ENR  (RCC_BASE + 0x134)

/* Global control (dual-core boot) */
#define RCC_GCR         (RCC_BASE + 0x0A0)   /* RM0399 §9.7.36 p477 */
#define RCC_GCR_WW1RSC  (1u << 0)
#define RCC_GCR_WW2RSC  (1u << 1)
#define RCC_GCR_BOOT_C1 (1u << 2)
#define RCC_GCR_BOOT_C2 (1u << 3)            /* Release CM4 from hold */

/* D2CKRDY: D2 domain clocks ready (RCC_CR bit 15, RM0399 §9.7.2) */
#define RCC_CR_D2CKRDY  (1u << 15)
#define RCC_CR_D1CKRDY  (1u << 14)

/* CPU2 (CM4) peripheral clock enable registers */
#define RCC_C2_AHB3ENR  (RCC_BASE + 0x194)
#define RCC_C2_AHB1ENR  (RCC_BASE + 0x198)
#define RCC_C2_AHB2ENR  (RCC_BASE + 0x19C)
#define RCC_C2_AHB4ENR  (RCC_BASE + 0x1A0)
#define RCC_C2_APB3ENR  (RCC_BASE + 0x1A4)
#define RCC_C2_APB1LENR (RCC_BASE + 0x1A8)
#define RCC_C2_APB1HENR (RCC_BASE + 0x1AC)
#define RCC_C2_APB2ENR  (RCC_BASE + 0x1B0)
#define RCC_C2_APB4ENR  (RCC_BASE + 0x1B4)

/* PLL / clock tree */
#define RCC_CR          (RCC_BASE + 0x000)
#define RCC_CFGR        (RCC_BASE + 0x010)
#define RCC_D1CFGR      (RCC_BASE + 0x018)   /* HPRE, D1PPRE, D1CPRE */
#define RCC_D2CFGR      (RCC_BASE + 0x01C)   /* D2PPRE1, D2PPRE2 */
#define RCC_D3CFGR      (RCC_BASE + 0x020)   /* D3PPRE */
#define RCC_PLLCKSELR   (RCC_BASE + 0x028)   /* PLL source + DIVM1/2/3 */
#define RCC_PLLCFGR     (RCC_BASE + 0x02C)   /* VCO range, output enables */
#define RCC_PLL1DIVR    (RCC_BASE + 0x030)   /* DIVN1, DIVP1, DIVQ1, DIVR1 */
#define RCC_PLL2DIVR    (RCC_BASE + 0x038)   /* DIVN2, DIVP2, DIVQ2, DIVR2 */
#define RCC_PLL3DIVR    (RCC_BASE + 0x040)   /* DIVN3, DIVP3, DIVQ3, DIVR3 */

/* RCC_CR bit positions (RM0399 §9.7.2 p415) */
#define RCC_CR_HSION    (1u <<  0)
#define RCC_CR_HSIRDY   (1u <<  2)
#define RCC_CR_HSEON    (1u << 16)
#define RCC_CR_HSERDY   (1u << 17)
#define RCC_CR_PLL1ON   (1u << 24)
#define RCC_CR_PLL1RDY  (1u << 25)
#define RCC_CR_PLL2ON   (1u << 26)
#define RCC_CR_PLL2RDY  (1u << 27)
#define RCC_CR_PLL3ON   (1u << 28)
#define RCC_CR_PLL3RDY  (1u << 29)

/* ---------- PWR (0x5802_4800) -------------------------------------------- */
#define PWR_BASE        0x58024800UL
#define PWR_CR3         (PWR_BASE + 0x0C)    /* supply config (SMPSEN, LDOEN) */
#define PWR_CSR1        (PWR_BASE + 0x04)    /* SMPSRDY status */
#define PWR_D3CR        (PWR_BASE + 0x18)    /* VOS, VOSRDY */
#define PWR_CPU1CR      (PWR_BASE + 0x010)   /* CPU1 power control (RM0399 §7.8.5) */
#define PWR_CPU2CR      (PWR_BASE + 0x014)   /* CPU2 power control (RM0399 §7.8.6) */

/* RM0399 Rev 4, §7.8.4 (p329-330): PWR_CR3 bit map
 * Reset value 0x0000_0006 → SDEN=1, LDOEN=1 (supply config ID 0).
 * Bits [3:1] are write-once after POR; invalid writes lock the register. */
#define PWR_CR3_BREN    (1u << 0)   /* Backup regulator enable */
#define PWR_CR3_LDOEN   (1u << 1)   /* LDO enable (write-once) */
#define PWR_CR3_SDEN    (1u << 2)   /* SMPS step-down enable (write-once) */
#define PWR_CR3_SDEXTHP (1u << 3)   /* SMPS forced ON, high-power (write-once) */

/* RM0399 §7.8.2 (p327): PWR_CSR1 bit map
 * Bit 16 is AVDO (analog voltage detector), NOT SMPSRDY.
 * There is no SMPSRDY in CSR1 on the H747. */
#define PWR_CSR1_PVDO       (1u << 4)
#define PWR_CSR1_ACTVOSRDY  (1u << 13)  /* VOS + SDLEVEL levels valid */
#define PWR_CSR1_AVDO       (1u << 16)  /* Analog voltage detector output */

/* RM0399 §7.8.6: PWR_D3CR */
#define PWR_D3CR_VOSRDY (1u << 13)

/* ---------- Flash interface (for wait-state config) ----------------------- */
#define FLASH_BASE      0x52002000UL
#define FLASH_ACR       (FLASH_BASE + 0x000)

/* ---------- FMC (0x5200_4000) -------------------------------------------- */
/* FMC base = 0x52004000 (RM0399 §24.9, Table 142) */
#define FMC_BCR1        0x52004000UL         /* Bank1 chip-select control */
#define FMC_SDCR1       0x52004140UL         /* SDRAM bank1 control (offset 0x140) */
#define FMC_SDCR2       0x52004144UL         /* SDRAM bank2 control (offset 0x144) */
#define FMC_SDTR1       0x52004148UL         /* SDRAM bank1 timing (offset 0x148) */
#define FMC_SDTR2       0x5200414CUL         /* SDRAM bank2 timing (offset 0x14C) */
#define FMC_SDCMR       0x52004150UL         /* SDRAM command mode (offset 0x150) */
#define FMC_SDRTR       0x52004154UL         /* SDRAM refresh timer (offset 0x154) */
#define FMC_SDSR        0x52004158UL         /* SDRAM status (offset 0x158) */

/* FMC_BCR1 bit */
#define FMC_BCR1_FMCEN  (1u << 31)

/* FMC_SDSR BUSY flag */
#define FMC_SDSR_BUSY   (1u << 5)

/* ---------- MPU (Cortex-M System Control Block, 0xE000_ED90) -------------- */
#define MPU_TYPE        0xE000ED90UL
#define MPU_CTRL        0xE000ED94UL
#define MPU_RNR         0xE000ED98UL
#define MPU_RBAR        0xE000ED9CUL
#define MPU_RASR        0xE000EDA0UL

#define MPU_CTRL_ENABLE      (1u << 0)
#define MPU_CTRL_PRIVDEFENA  (1u << 2)

/* ---------- NVIC (Cortex-M7 private peripheral bus) ---------------------- */
#define NVIC_ICER_BASE  0xE000E180UL

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

/* ---------- Memory barriers (Cortex-M7) ---------------------------------- */
static inline void __dsb(void) { __asm volatile ("dsb sy" ::: "memory"); }
static inline void __isb(void) { __asm volatile ("isb sy" ::: "memory"); }
static inline void __nop(void) { __asm volatile ("nop"); }

#endif /* STM32H747XI_H */
