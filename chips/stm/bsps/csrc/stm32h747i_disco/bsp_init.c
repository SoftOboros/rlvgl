/* Top-level BSP initialisation for STM32H747I-DISCO (CM7).
 *
 * SPDX-FileCopyrightText: 2025 Softoboros Technology, Inc.
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * c_bsp_init() is called from the thin Rust #[entry] after heap
 * initialisation.  It sequences all hardware bring-up and then transfers
 * control to the Rust application entry point rlvgl_app_main().
 *
 * Callers only need to include this file's declarations; each subsystem
 * has its own translation unit so individual functions can be called or
 * replaced independently.
 */

#include "stm32h747xi.h"

/* Forward declarations for subsystem init functions */
void c_system_init_mpu(void);
void c_system_init_power(void);
void c_system_init_clocks(void);
void c_system_enable_d1_peripherals(void);

void c_gpio_fmc_enable_clocks(void);
void c_gpio_fmc_configure_pins(void);

void c_sdram_init(void);

/* Peripheral GPIO init (individual translation units) */
void c_i2c4_enable_gpio_clocks(void);
void c_i2c4_configure_pins(void);
void c_usart1_enable_gpio_clocks(void);
void c_usart1_configure_pins(void);
void c_spi2_enable_gpio_clocks(void);
void c_spi2_configure_pins(void);
void c_spi5_enable_gpio_clocks(void);
void c_spi5_configure_pins(void);
void c_uart8_enable_gpio_clocks(void);
void c_uart8_configure_pins(void);

/* Rust application entry — never returns */
extern void rlvgl_app_main(void);

/* --------------------------------------------------------------------------
 * Control GPIO: panel reset (PG3), backlight (PJ6), touch INT (PK7)
 * --------------------------------------------------------------------------
 * GPIOG is already clocked by c_gpio_fmc_enable_clocks().
 * GPIOJ and GPIOK are enabled here.
 */
static void c_gpio_control_init(void)
{
    /* Enable GPIOJ (bit 9) and GPIOK (bit 10) clocks */
    REG32(RCC_AHB4ENR) |= (1u << 9) | (1u << 10);
    (void)REG32(RCC_AHB4ENR);

    /* PG3 – panel reset: output push-pull, medium speed, no pull */
    {
        uint32_t base = GPIOG_BASE;
        /* MODER bits 7:6 = 01 (output) */
        REG32(base + GPIO_MODER) = (REG32(base + GPIO_MODER) & ~(3u << 6)) | (1u << 6);
        /* OTYPER bit 3 = 0 (push-pull) */
        REG32(base + GPIO_OTYPER) &= ~(1u << 3);
        /* OSPEEDR bits 7:6 = 01 (medium) */
        REG32(base + GPIO_OSPEEDR) = (REG32(base + GPIO_OSPEEDR) & ~(3u << 6)) | (1u << 6);
        /* PUPDR bits 7:6 = 00 (no pull) */
        REG32(base + GPIO_PUPDR) &= ~(3u << 6);
        /* Initial state: reset asserted (low) */
        GPIO_CLEAR(base, 3);
    }

    /* PJ6 – backlight: output push-pull, medium speed, no pull, initially off */
    {
        uint32_t base = GPIOJ_BASE;
        REG32(base + GPIO_MODER) = (REG32(base + GPIO_MODER) & ~(3u << 12)) | (1u << 12);
        REG32(base + GPIO_OTYPER) &= ~(1u << 6);
        REG32(base + GPIO_OSPEEDR) = (REG32(base + GPIO_OSPEEDR) & ~(3u << 12)) | (1u << 12);
        REG32(base + GPIO_PUPDR) &= ~(3u << 12);
        GPIO_CLEAR(base, 6);
    }

    /* PK7 – touch INT: floating input (MODER = 00, PUPDR = 00) */
    {
        uint32_t base = GPIOK_BASE;
        REG32(base + GPIO_MODER) &= ~(3u << 14);
        REG32(base + GPIO_PUPDR) &= ~(3u << 14);
    }
}

/* --------------------------------------------------------------------------
 * Main BSP entry point
 * -------------------------------------------------------------------------- */
void c_bsp_init(void)
{
    /* 1. Memory protection unit */
    c_system_init_mpu();

    /* 2. Power supply (SMPS + VOS1) */
    c_system_init_power();

    /* 3. Clock tree (HSE 25 MHz → 400 MHz SYSCLK) */
    c_system_init_clocks();

    /* 4. D1 peripheral clocks (LTDC, DMA2D, DSI, FMC) */
    c_system_enable_d1_peripherals();

    /* 5. FMC SDRAM GPIO */
    c_gpio_fmc_enable_clocks();
    c_gpio_fmc_configure_pins();

    /* 6. SDRAM controller */
    c_sdram_init();

    /* 7. Display/input control GPIO */
    c_gpio_control_init();

    /* 8. Peripheral GPIO (I2C4, USART1, SPI2, SPI5, UART8) */
    c_i2c4_enable_gpio_clocks();
    c_i2c4_configure_pins();
    c_usart1_enable_gpio_clocks();
    c_usart1_configure_pins();
    c_spi2_enable_gpio_clocks();
    c_spi2_configure_pins();
    c_spi5_enable_gpio_clocks();
    c_spi5_configure_pins();
    c_uart8_enable_gpio_clocks();
    c_uart8_configure_pins();

    /* 9. Release CM4 from hardware hold (RM0399 §9.7.36, Table 58).
     *    BCM4 option byte default = 0 → CM4 stays held until BOOT_C2 = 1. */
    REG32(RCC_GCR) |= RCC_GCR_BOOT_C2;
    __dsb();

    /* 10. Hand control to the Rust application */
    rlvgl_app_main();

    /* Should never reach here */
    for (;;) {}
}
