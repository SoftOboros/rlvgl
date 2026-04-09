/* STM32H747I-DISCO system initialisation: MPU, power supply, clock tree,
 * and D1 peripheral clock enables.
 *
 * SPDX-FileCopyrightText: 2025 Softoboros Technology, Inc.
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * Target: STM32H747XIHx CM7 core
 * Crystal: HSE 25 MHz
 * Target clocks:
 *   SYSCLK  = 400 MHz  (PLL1_P, D1CPRE=1)
 *   HCLK    = 200 MHz  (HPRE=2)
 *   APBx    = 100 MHz  (PPREx=2)
 *   PLL2_R  = 150 MHz  (FMC kernel clock, optional)
 *   PLL3_R  =  32 MHz  (DSI pixel clock)
 */

#include "stm32h747xi.h"

/* =========================================================================
 * MPU – 6 regions matching the Rust bringup configuration
 * =========================================================================
 *
 * RASR encoding (Cortex-M7 MPU):
 *   bit  0      : ENABLE
 *   bits  8: 1  : SIZE  (region size = 2^(SIZE+1) bytes)
 *   bits 26:24  : AP    (0b011 = full RW, privileged+user)
 *   bits 21:19  : TEX
 *   bit  18     : S     (shareable)
 *   bit  17     : C     (cacheable)
 *   bit  16     : B     (bufferable)
 *   bit  28     : XN    (execute never)
 *
 * AP_FULL_ACCESS = 0b011 << 24 = 0x03000000
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

void c_system_init_mpu(void)
{
    /* Disable MPU before changing regions */
    REG32(MPU_CTRL) = 0;
    __dsb();
    __isb();

    /* Region 0: Flash  0x0800_0000  2 MB  (cacheable, bufferable) */
    REG32(MPU_RNR)  = 0;
    REG32(MPU_RBAR) = 0x08000000u;
    REG32(MPU_RASR) = mpu_rasr(20, AP_FULL, 0, 0, 1, 1, 0);

    /* Region 1: DTCM   0x2000_0000  128 KB (cacheable, XN) */
    REG32(MPU_RNR)  = 1;
    REG32(MPU_RBAR) = 0x20000000u;
    REG32(MPU_RASR) = mpu_rasr(16, AP_FULL, 0, 0, 1, 1, 1);

    /* Region 2: D1 SRAM 0x2400_0000 512 KB (shareable, cacheable, XN) */
    REG32(MPU_RNR)  = 2;
    REG32(MPU_RBAR) = 0x24000000u;
    REG32(MPU_RASR) = mpu_rasr(18, AP_FULL, 0, 1, 1, 1, 1);

    /* Region 3: Mailbox 0x3004_7000 2 KB  (shared D2 SRAM, non-cacheable, XN) */
    REG32(MPU_RNR)  = 3;
    REG32(MPU_RBAR) = 0x30047000u;
    REG32(MPU_RASR) = mpu_rasr(11, AP_FULL, 0, 1, 0, 0, 1);

    /* Region 4: D3 SRAM 0x3800_0000 64 KB (shareable, cacheable, XN) */
    REG32(MPU_RNR)  = 4;
    REG32(MPU_RBAR) = 0x38000000u;
    REG32(MPU_RASR) = mpu_rasr(15, AP_FULL, 0, 1, 1, 1, 1);

    /* Region 5: SDRAM  0xC000_0000 512 MB (shareable, TEX=1, non-cacheable, XN) */
    REG32(MPU_RNR)  = 5;
    REG32(MPU_RBAR) = 0xC0000000u;
    REG32(MPU_RASR) = mpu_rasr(24, AP_FULL, 1, 1, 0, 0, 1);

    /* Enable MPU with privileged-default background region */
    __dsb();
    REG32(MPU_CTRL) = MPU_CTRL_ENABLE | MPU_CTRL_PRIVDEFENA;
    __dsb();
    __isb();
}

/* =========================================================================
 * Power supply → VOS1
 * =========================================================================
 *
 * DO NOT WRITE PWR_CR3.
 *
 * RM0399 Rev 4 §7.8.4 documents the POR reset value as 0x0000_0006
 * (SDEN=1, LDOEN=1, Config 0).  The actual STM32H747I-DISCO silicon
 * (STM32H747XIHx BGA240) resets to a DIFFERENT value:
 *
 *   CR3  = 0x0001_0044  →  SDEN=1, LDOEN=0, SDEXTRDY=1  (Config 2)
 *   CSR1 = 0x0000_6000  →  ACTVOSRDY=1, ACTVOS=01 (VOS3)
 *
 * Verified empirically via probe-rs read immediately after USB power
 * cycle with core held in reset (connect-under-reset).
 *
 * The POR default is Config 2 (Direct SMPS, LDO bypassed).  ACTVOSRDY
 * is already 1.  Writing CR3 — even with a "valid" value — changes
 * LDOEN and corrupts the supply config.  The write-once mechanism then
 * locks the bad value, and ACTVOSRDY drops to 0 permanently until the
 * next USB power cycle.
 *
 * At VOS1 the CM7 can run at up to 400 MHz.
 */
void c_system_init_power(void)
{
    /* Enable SYSCFG clock (RCC_APB4ENR bit 1, RM0399 §9.7.47 p507). */
    REG32(RCC_APB4ENR) |= (1u << 1);
    (void)REG32(RCC_APB4ENR);

    /* Configure supply: Direct SMPS (Config 2, Table 33 p275).
     *
     * POR default CR3 = 0x46 (SDEN=1, LDOEN=1, SDLEVEL=00, bit 6=1).
     * ACTVOSRDY=0 at POR — software MUST write CR3 once to configure
     * the supply before ACTVOSRDY will go high (RM0399 p277/p329).
     *
     * Clear LDOEN to select Config 2 (Direct SMPS, LDO bypassed).
     * Preserve bit 6 and SDEN.  On debug reset the register retains
     * our value (write-once survives pin reset) and this write is
     * harmlessly ignored. */
    REG32(PWR_CR3) = (REG32(PWR_CR3) & ~PWR_CR3_LDOEN) | PWR_CR3_SDEN;

    /* Wait for supply to stabilise (RM0399 p279) */
    while (!(REG32(PWR_CSR1) & PWR_CSR1_ACTVOSRDY)) {}

    /* Select VOS1: D3CR bits 15:14 = 0b11 (RM0399 §7.8.6) */
    REG32(PWR_D3CR) = (REG32(PWR_D3CR) & ~(3u << 14)) | (3u << 14);

    /* Wait for voltage scaling ready (D3CR bit 13 VOSRDY) */
    while (!(REG32(PWR_D3CR) & PWR_D3CR_VOSRDY)) {}
}

/* =========================================================================
 * Clock tree: HSE 25 MHz → PLL1 400 MHz SYSCLK, PLL2 150 MHz, PLL3 32 MHz
 * =========================================================================
 *
 * PLL input dividers (DIVM): all set to /5 → VCO reference = 5 MHz
 * PLL1: N=160, P=2 → VCO=800 MHz, SYSCLK=400 MHz  (wide VCO: 192-836 MHz)
 * PLL2: N=60,  R=2 → VCO=300 MHz, PLL2_R=150 MHz  (medium VCO: 150-420 MHz)
 * PLL3: N=64,  R=10 → VCO=320 MHz, PLL3_R=32 MHz  (medium VCO)
 *
 * Bus prescalers:
 *   D1CPRE = 1  → D1 CPU clock = 400 MHz
 *   HPRE   = 2  → HCLK         = 200 MHz
 *   D1PPRE = 2  → APB3          = 100 MHz
 *   D2PPRE1= 2  → APB1          = 100 MHz
 *   D2PPRE2= 2  → APB2          = 100 MHz
 *   D3PPRE = 2  → APB4          = 100 MHz
 *
 * Flash wait states: 4 WS required at 400 MHz / VOS1.
 */

/* RCC_PLLCKSELR (RM0399 §9.7.10):
 *   PLLSRC  bits  1:0
 *   DIVM1   bits  9:4
 *   DIVM2   bits 17:12
 *   DIVM3   bits 25:20
 * Source=HSE, all DIVM=5 → ref = HSE/5 = 5 MHz */
#define PLLCKSELR_VAL   ((2u)        |   /* PLLSRC = HSE */  \
                         (5u <<  4)  |   /* DIVM1  = 5   */  \
                         (5u << 12)  |   /* DIVM2  = 5   */  \
                         (5u << 20))     /* DIVM3  = 5   */

/* RCC_PLLCFGR (RM0399 §9.7.11, p431):
 *   Bits 24-16: DIVRxEN/DIVQxEN/DIVPxEN (output enables)
 *   Bits 11-0:  PLLxRGE/PLLxVCOSEL/PLLxFRACEN (VCO config)
 *
 *   PLL1: P/Q/R enabled, RGE=0b10 (4-8 MHz), VCOSEL=0 (wide 192-960 MHz)
 *   PLL2: R enabled,      RGE=0b10,           VCOSEL=1 (medium 150-420 MHz)
 *   PLL3: R enabled,      RGE=0b10,           VCOSEL=1 (medium 150-420 MHz) */
#define PLLCFGR_VAL     (/* PLL1: FRACEN=0, VCOSEL=0(wide), RGE=10 */     \
                         (0u <<  0)  |   /* PLL1FRACEN  */                 \
                         (0u <<  1)  |   /* PLL1VCOSEL = wide */           \
                         (2u <<  2)  |   /* PLL1RGE = 4-8 MHz */           \
                         /* PLL2: FRACEN=0, VCOSEL=1(medium), RGE=10 */    \
                         (0u <<  4)  |   /* PLL2FRACEN  */                 \
                         (1u <<  5)  |   /* PLL2VCOSEL = medium */         \
                         (2u <<  6)  |   /* PLL2RGE = 4-8 MHz */           \
                         /* PLL3: FRACEN=0, VCOSEL=0(wide 192-960), RGE=10 */\
                         (0u <<  8)  |   /* PLL3FRACEN  */                 \
                         (0u <<  9)  |   /* PLL3VCOSEL = wide (192-960) */ \
                         (2u << 10)  |   /* PLL3RGE = 4-8 MHz */           \
                         /* Output enables */                              \
                         (1u << 16)  |   /* DIVP1EN */                     \
                         (1u << 17)  |   /* DIVQ1EN */                     \
                         (1u << 18)  |   /* DIVR1EN */                     \
                         (1u << 21)  |   /* DIVR2EN */                     \
                         (1u << 24))     /* DIVR3EN */

/* RCC_PLL1DIVR: N=160, P=2, Q=4, R=2 (all written as value-1) */
#define PLL1DIVR_VAL    ((1u << 24) |   /* DIVR1-1 = 1 (R=2)   */  \
                         (3u << 16) |   /* DIVQ1-1 = 3 (Q=4)   */  \
                         (1u <<  9) |   /* DIVP1-1 = 1 (P=2)   */  \
                         (159u))        /* DIVN1-1 = 159 (N=160) */

/* RCC_PLL2DIVR: N=60, P=2, Q=4, R=2 (all written as value-1) */
#define PLL2DIVR_VAL    ((1u << 24) |   /* DIVR2-1 = 1 (R=2)   */  \
                         (3u << 16) |   /* DIVQ2-1 = 3 (Q=4)   */  \
                         (1u <<  9) |   /* DIVP2-1 = 1 (P=2)   */  \
                         (59u))         /* DIVN2-1 = 59 (N=60)  */

/* RCC_PLL3DIVR: N=64, P=10, Q=8, R=10 (all written as value-1)
 * VCO = (HSE/5) × 64 = 320 MHz (wide VCO: 192-960)
 * PLL3_R = 320/10 = 32 MHz (LTDC pixel clock)
 * TODO: change to N=132,R=24 (27.5 MHz) once LTDC aliasing bug is resolved */
#define PLL3DIVR_VAL    ((9u << 24) |   /* DIVR3-1 = 9 (R=10)  */  \
                         (7u << 16) |   /* DIVQ3-1 = 7 (Q=8)   */  \
                         (9u <<  9) |   /* DIVP3-1 = 9 (P=10)  */  \
                         (63u))         /* DIVN3-1 = 63 (N=64)  */

/* Bus prescaler register values */
/* D1CFGR: D1CPRE=0 (no CPU div), D1PPRE=0b100 (/2 → APB3=100MHz), HPRE=0b1000 (/2 → HCLK=200MHz) */
#define D1CFGR_VAL      ((0u << 8) | (4u << 4) | (8u))
/* D2CFGR: D2PPRE2=0b100 (/2 → APB2=100MHz), D2PPRE1=0b100 (/2 → APB1=100MHz) */
#define D2CFGR_VAL      ((4u << 4) | (4u))
/* D3CFGR: D3PPRE=0b100 (/2 → APB4=100MHz) */
#define D3CFGR_VAL      (4u)

void c_system_init_clocks(void)
{
    /* 1. Set flash wait states for 400 MHz before raising the clock */
    REG32(FLASH_ACR) = (REG32(FLASH_ACR) & ~0xFu) | 4u;
    (void)REG32(FLASH_ACR);

    /* 2. Start HSE oscillator */
    REG32(RCC_CR) |= RCC_CR_HSEON;
    while (!(REG32(RCC_CR) & RCC_CR_HSERDY)) {}

    /* 3. Configure PLL source and input dividers */
    REG32(RCC_PLLCKSELR) = PLLCKSELR_VAL;

    /* 4. Configure VCO ranges and output enables */
    REG32(RCC_PLLCFGR) = PLLCFGR_VAL;

    /* 5. Load PLL divider ratios */
    REG32(RCC_PLL1DIVR) = PLL1DIVR_VAL;
    REG32(RCC_PLL2DIVR) = PLL2DIVR_VAL;
    REG32(RCC_PLL3DIVR) = PLL3DIVR_VAL;

    /* 6. Enable PLL1, wait ready */
    REG32(RCC_CR) |= RCC_CR_PLL1ON;
    while (!(REG32(RCC_CR) & RCC_CR_PLL1RDY)) {}

    /* 7. Enable PLL2, wait ready */
    REG32(RCC_CR) |= RCC_CR_PLL2ON;
    while (!(REG32(RCC_CR) & RCC_CR_PLL2RDY)) {}

    /* 7b. Enable PLL3 (DSI pixel clock: PLL3_R = 32 MHz), wait ready */
    REG32(RCC_CR) |= RCC_CR_PLL3ON;
    while (!(REG32(RCC_CR) & RCC_CR_PLL3RDY)) {}

    /* 8. Set bus prescalers */
    REG32(RCC_D1CFGR) = D1CFGR_VAL;
    REG32(RCC_D2CFGR) = D2CFGR_VAL;
    REG32(RCC_D3CFGR) = D3CFGR_VAL;

    /* 9. Switch SYSCLK to PLL1 (SW = 0b011) */
    REG32(RCC_CFGR) = (REG32(RCC_CFGR) & ~0x7u) | 0x3u;

    /* 10. Wait for clock switch confirmation (SWS bits 5:3 = 011) */
    while ((REG32(RCC_CFGR) & (0x7u << 3)) != (0x3u << 3)) {}
}

/* =========================================================================
 * D1 peripheral clock enables: LTDC, DMA2D, DSI, FMC
 * =========================================================================
 * These must be enabled before the Rust side tries to configure them.
 */
void c_system_enable_d1_peripherals(void)
{
    /* APB3: LTDC (bit 3), DSI (bit 4) */
    REG32(RCC_APB3ENR) |= (1u << 3) | (1u << 4);

    /* AHB3: DMA2D (bit 4), FMC (bit 12) */
    REG32(RCC_AHB3ENR) |= (1u << 4) | (1u << 12);

    /* CM7 domain mirror for FMC */
    REG32(RCC_C1_AHB3ENR) |= (1u << 12);

    /* Read-back to flush peripheral clock enable pipeline */
    (void)REG32(RCC_APB3ENR);
    (void)REG32(RCC_AHB3ENR);
}
