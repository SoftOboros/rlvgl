/* FMC SDRAM controller initialisation for STM32H747I-DISCO.
 *
 * SPDX-FileCopyrightText: 2025 Softoboros Technology, Inc.
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * Device: IS42S32800G-6BLI (32 Mbit × 8 banks, 32-bit bus, 100 MHz max)
 * SDRAM kernel clock: HCLK3 / 2 = 100 MHz  (SDCLK=0b10 in SDCR)
 *
 * Timing values at 100 MHz (10 ns per cycle), all written as (cycles - 1):
 *   TMRD  = 1  (2 cycles,  tMRD ≥ 2 clk)
 *   TXSR  = 6  (7 cycles,  tXSR ≤ 70 ns)
 *   TRAS  = 4  (5 cycles,  tRAS ≥ 42 ns)
 *   TRC   = 6  (7 cycles,  tRC  ≥ 60 ns)
 *   TWR   = 1  (2 cycles)
 *   TRP   = 1  (2 cycles,  tRP  ≥ 15 ns)
 *   TRCD  = 1  (2 cycles,  tRCD ≥ 15 ns)
 *
 * Refresh: 64 ms / 4096 rows at 100 MHz → counter = 1563; use 566 (conservative).
 *
 * SDCR1 bit layout (RM0399 §24.7.3):
 *   NC[1:0]   = 01  (9-bit column)
 *   NR[1:0]   = 01  (12-bit row)
 *   MWID[1:0] = 10  (32-bit bus)
 *   NB        = 1   (4 banks)
 *   CAS[1:0]  = 11  (CAS latency 3)
 *   WP        = 0   (write allowed)
 *   SDCLK[1:0]= 10  (HCLK / 2)
 *   RBURST    = 1   (burst read)
 *   RPIPE[1:0]= 00  (no pipeline delay)
 */

#include "stm32h747xi.h"

/* Small busy-wait: scale must be calibrated for 400 MHz CPU */
static void sdram_delay(uint32_t cycles)
{
    volatile uint32_t n = cycles;
    while (n--) { __nop(); }
}

static void sdram_wait_busy(void)
{
    while (REG32(FMC_SDSR) & FMC_SDSR_BUSY) {}
}

/* Issue an SDRAM command to bank 1 (CTB1=1, CTB2=0) */
static void sdram_cmd(uint32_t mode, uint32_t nrfs, uint32_t mrd)
{
    sdram_wait_busy();
    REG32(FMC_SDCMR) = ((mrd  & 0x1FFFu) << 9)
                      | ((nrfs & 0xFu)    << 5)
                      | (1u               << 4)   /* CTB1 */
                      | (mode & 0x7u);
    sdram_wait_busy();
}

/* SDCR1: NC=01, NR=01, MWID=10, NB=1, CAS=11, WP=0, SDCLK=10, RBURST=1, RPIPE=00 */
#define SDCR1_VAL   ((0u  << 13) |   /* RPIPE = 0  */  \
                     (1u  << 12) |   /* RBURST= 1  */  \
                     (2u  << 10) |   /* SDCLK = /2 */  \
                     (0u  <<  9) |   /* WP    = 0  */  \
                     (3u  <<  7) |   /* CAS   = 3  */  \
                     (1u  <<  6) |   /* NB    = 4  */  \
                     (2u  <<  4) |   /* MWID  = 32 */  \
                     (1u  <<  2) |   /* NR    = 12 */  \
                     (1u  <<  0))    /* NC    = 9  */

/* SDTR1: TRCD=1, TRP=1, TWR=1, TRC=6, TRAS=4, TXSR=6, TMRD=1 */
#define SDTR1_VAL   ((1u << 24) |   /* TRCD */  \
                     (1u << 20) |   /* TRP  */  \
                     (1u << 16) |   /* TWR  */  \
                     (6u << 12) |   /* TRC  */  \
                     (4u <<  8) |   /* TRAS */  \
                     (6u <<  4) |   /* TXSR */  \
                     (1u <<  0))    /* TMRD */

/* Mode register: burst=1, sequential, CAS=3, single-write */
#define SDRAM_MODE_REG  0x0230u

/* Refresh count for 64ms/4096 rows at 100MHz SDRAM clock (conservative) */
#define SDRAM_REFRESH   566u

void c_sdram_init(void)
{
    /* Enable FMC controller (BCR1 bit 31) */
    REG32(FMC_BCR1) |= FMC_BCR1_FMCEN;

    /* Program control and timing registers */
    REG32(FMC_SDCR1) = SDCR1_VAL;
    REG32(FMC_SDTR1) = SDTR1_VAL;

    /* SDRAM initialisation sequence per JEDEC / RM0399 §24.7.4 */

    /* Step 1: Clock configuration enable */
    sdram_cmd(0x1u, 0u, 0u);

    /* Step 2: Wait ≥ 100 µs (at 400 MHz CPU: ~40 000 cycles) */
    sdram_delay(40000u);

    /* Step 3: Precharge all banks */
    sdram_cmd(0x2u, 0u, 0u);

    /* Step 4: 8× auto-refresh (NRFS = 7 → 7+1 = 8 cycles) */
    sdram_cmd(0x3u, 7u, 0u);

    /* Step 5: Load mode register */
    sdram_cmd(0x4u, 0u, SDRAM_MODE_REG);

    /* Step 6: Exit: normal mode */
    sdram_cmd(0x0u, 0u, 0u);

    /* Program refresh timer (SDRTR COUNT field = bits 13:1) */
    REG32(FMC_SDRTR) = SDRAM_REFRESH << 1;

    /* Final busy poll */
    sdram_wait_busy();
}
