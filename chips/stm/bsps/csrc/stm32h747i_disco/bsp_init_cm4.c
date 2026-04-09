/* Top-level BSP initialisation for STM32H747I-DISCO (CM4).
 *
 * SPDX-FileCopyrightText: 2025 Softoboros Technology, Inc.
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * c_bsp_init_cm4() is called from the Rust CM4 #[entry] after heap
 * initialisation.  CM7 has already configured power, clocks, and SDRAM.
 * CM4 only needs its own MPU, D2 clock readiness, and peripheral enables.
 *
 * All register addresses verified against RM0399 Rev 4.
 */

#include "stm32h747xi.h"

/* Forward declarations for CM4 subsystem init functions */
void c_system_init_mpu_cm4(void);
void c_system_wait_d2_clocks(void);
void c_system_enable_d2_peripherals_cm4(void);

/* Rust CM4 application entry — never returns */
extern void rlvgl_cm4_main(void);

/* --------------------------------------------------------------------------
 * Main CM4 BSP entry point
 * -------------------------------------------------------------------------- */
void c_bsp_init_cm4(void)
{
    /* 1. Configure CM4's own MPU */
    c_system_init_mpu_cm4();

    /* 2. Wait for D2 domain clocks (CM7 drives the clock tree) */
    c_system_wait_d2_clocks();

    /* 3. Enable peripheral clocks via CM4's RCC_C2_* registers */
    c_system_enable_d2_peripherals_cm4();

    /* 4. Hand control to the Rust CM4 application */
    rlvgl_cm4_main();

    /* Should never reach here */
    for (;;) {}
}
