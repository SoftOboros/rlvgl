/* STM32H747I-DISCO CM4 system initialisation: MPU and D2 domain clock enables.
 *
 * SPDX-FileCopyrightText: 2025 Softoboros Technology, Inc.
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * Target: STM32H747XIHx CM4 core
 *
 * CM4 does NOT configure power supply or PLL — CM7 has already done that.
 * CM4 must:
 *   1. Configure its own MPU (separate from CM7's MPU)
 *   2. Wait for D2 domain clocks to be ready (D2CKRDY in RCC_CR)
 *   3. Enable peripheral clocks via RCC_C2_* registers
 *
 * All register addresses verified against RM0399 Rev 4.
 */

#include "stm32h747xi.h"

/* =========================================================================
 * MPU – CM4 has its own MPU, independent of CM7's
 * =========================================================================
 *
 * RASR encoding is identical to CM7 (both are ARMv7-M MPU).
 */
#define AP_FULL  0x03000000u

static inline uint32_t mpu_rasr(uint32_t size_field, uint32_t ap,
                                 uint32_t tex, uint32_t s,
                                 uint32_t c,   uint32_t b,
                                 uint32_t xn)
{
    return 1u
         | (size_field << 1)
         | ap
         | (tex << 19)
         | (s   << 18)
         | (c   << 17)
         | (b   << 16)
         | (xn  << 28);
}

void c_system_init_mpu_cm4(void)
{
    /* Disable MPU before changing regions */
    REG32(MPU_CTRL) = 0;
    __dsb();
    __isb();

    /* Region 0: Flash Bank 2  0x0810_0000  1 MB  (cacheable, bufferable) */
    REG32(MPU_RNR)  = 0;
    REG32(MPU_RBAR) = 0x08100000u;
    REG32(MPU_RASR) = mpu_rasr(19, AP_FULL, 0, 0, 1, 1, 0);  /* 2^(19+1) = 1MB */

    /* Region 1: D2 SRAM  0x3000_0000  256 KB (shareable, cacheable, XN)
     * CM4 primary RAM: SRAM1(128K) + SRAM2(128K) */
    REG32(MPU_RNR)  = 1;
    REG32(MPU_RBAR) = 0x30000000u;
    REG32(MPU_RASR) = mpu_rasr(17, AP_FULL, 0, 1, 1, 1, 1);  /* 2^(17+1) = 256KB */

    /* Region 2: Mailbox  0x3004_7000  2 KB  (shared, non-cacheable, XN)
     * Must be non-cacheable for cross-core coherency */
    REG32(MPU_RNR)  = 2;
    REG32(MPU_RBAR) = 0x30047000u;
    REG32(MPU_RASR) = mpu_rasr(11, AP_FULL, 0, 1, 0, 0, 1);  /* 2^(11+1) = 4KB (covers 1K) */

    /* Region 3: D1 AXI SRAM CM4 slice  0x2406_0000  128 KB (shareable, cacheable, XN) */
    REG32(MPU_RNR)  = 3;
    REG32(MPU_RBAR) = 0x24060000u;
    REG32(MPU_RASR) = mpu_rasr(16, AP_FULL, 0, 1, 1, 1, 1);  /* 2^(16+1) = 128KB */

    /* Region 4: D3 SRAM4  0x3800_0000  64 KB (shareable, cacheable, XN) */
    REG32(MPU_RNR)  = 4;
    REG32(MPU_RBAR) = 0x38000000u;
    REG32(MPU_RASR) = mpu_rasr(15, AP_FULL, 0, 1, 1, 1, 1);  /* 2^(15+1) = 64KB */

    /* Enable MPU with default memory map for privileged access */
    __dsb();
    __isb();
    REG32(MPU_CTRL) = MPU_CTRL_ENABLE | MPU_CTRL_PRIVDEFENA;
    __dsb();
    __isb();
}

/* =========================================================================
 * Wait for D2 domain clocks ready
 * =========================================================================
 * RM0399 §9.3: Before enabling D2 peripheral clocks, wait for D2CKRDY.
 * D2CKRDY is RCC_CR bit 15 (RM0399 §9.7.2 p415).
 */
void c_system_wait_d2_clocks(void)
{
    while (!(REG32(RCC_CR) & RCC_CR_D2CKRDY)) {
        __nop();
    }
}

/* =========================================================================
 * Enable D2 peripheral clocks for CM4
 * =========================================================================
 * CM4 uses RCC_C2_* registers to allocate peripherals to itself.
 * Only enable what CM4 actually needs.
 *
 * RM0399 §9.5.10: A peripheral must be enabled by at least one CPU
 * via its C1_PERxEN or C2_PERxEN bit for clocks to be provided.
 */
void c_system_enable_d2_peripherals_cm4(void)
{
    /* GPIO clocks (AHB4): enable ports used by CM4 peripherals.
     * GPIO A-K bits 0-10 in RCC_C2_AHB4ENR.
     * Enable all for now — same as CM7 does. */
    REG32(RCC_C2_AHB4ENR) |= (1u << 0)   /* GPIOA */
                            | (1u << 1)   /* GPIOB */
                            | (1u << 2)   /* GPIOC */
                            | (1u << 3)   /* GPIOD */
                            | (1u << 4)   /* GPIOE */
                            | (1u << 7)   /* GPIOH */
                            | (1u << 8)   /* GPIOI */
                            ;
    (void)REG32(RCC_C2_AHB4ENR);  /* read-back for clock stabilisation */
}
