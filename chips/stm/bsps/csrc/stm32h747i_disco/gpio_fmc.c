/* FMC SDRAM GPIO pin configuration for STM32H747I-DISCO.
 *
 * SPDX-FileCopyrightText: 2025 Softoboros Technology, Inc.
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * All FMC address/data/control pins are configured as:
 *   Alternate function 12 (AF12)
 *   Very high output speed (OSPEEDR = 0b11)
 *   No pull-up/pull-down
 *   Push-pull output type
 *
 * Pin map (RM0399 / STM32H747I-DISCO schematic):
 *   PD:  D0(14), D1(15), D2(0),  D3(1),  D13(8), D14(9), D15(10)
 *   PE:  NBL0(0), NBL1(1), D4(7), D5(8), D6(9), D7(10), D8(11),
 *        D9(12), D10(13), D11(14), D12(15)
 *   PF:  A0(0), A1(1), A2(2), A3(3), A4(4), A5(5),
 *        SDNRAS(11), A6(12), A7(13), A8(14), A9(15)
 *   PG:  A10(0), A11(1), BA0(4), BA1(5), SDCLK(8), SDNCAS(15), A12(2)
 *   PH:  SDNE0(5), SDCKE0(6), SDNWE(3→actually 5 used below),
 *        D16(8), D17(9), D18(10), D19(11), D20(12), D21(13), D22(14), D23(15)
 *   PI:  D24(0), D25(1), D26(2), D27(3), NBL2(4), NBL3(5),
 *        D28(6), D29(7), D30(9), D31(10)
 */

#include "stm32h747xi.h"

/* Helper: configure a single GPIO pin for AF12, very-high speed, PP, no pull */
static void fmc_pin(uint32_t port_base, uint8_t pin)
{
    uint32_t shift2 = (uint32_t)pin * 2u;

    /* MODER: alternate function (0b10) */
    REG32(port_base + GPIO_MODER) =
        (REG32(port_base + GPIO_MODER) & ~(3u << shift2)) | (2u << shift2);

    /* OSPEEDR: very high speed (0b11) */
    REG32(port_base + GPIO_OSPEEDR) =
        (REG32(port_base + GPIO_OSPEEDR) & ~(3u << shift2)) | (3u << shift2);

    /* PUPDR: no pull (0b00) */
    REG32(port_base + GPIO_PUPDR) &= ~(3u << shift2);

    /* OTYPER: push-pull (bit = 0) */
    REG32(port_base + GPIO_OTYPER) &= ~(1u << pin);

    /* AFR: AF12 (0xC) in AFRL (pins 0-7) or AFRH (pins 8-15) */
    if (pin < 8u) {
        uint32_t s4 = (uint32_t)pin * 4u;
        REG32(port_base + GPIO_AFRL) =
            (REG32(port_base + GPIO_AFRL) & ~(0xFu << s4)) | (12u << s4);
    } else {
        uint32_t s4 = ((uint32_t)pin - 8u) * 4u;
        REG32(port_base + GPIO_AFRH) =
            (REG32(port_base + GPIO_AFRH) & ~(0xFu << s4)) | (12u << s4);
    }
}

void c_gpio_fmc_enable_clocks(void)
{
    /* Enable GPIO D, E, F, G, H, I clocks (AHB4ENR bits 3-8) */
    REG32(RCC_AHB4ENR) |= (1u<<3)|(1u<<4)|(1u<<5)|(1u<<6)|(1u<<7)|(1u<<8);
    (void)REG32(RCC_AHB4ENR);
}

void c_gpio_fmc_configure_pins(void)
{
    static const uint8_t pd_pins[] = { 0, 1, 8, 9, 10, 14, 15 };
    static const uint8_t pe_pins[] = { 0, 1, 7, 8, 9, 10, 11, 12, 13, 14, 15 };
    static const uint8_t pf_pins[] = { 0, 1, 2, 3, 4, 5, 11, 12, 13, 14, 15 };
    static const uint8_t pg_pins[] = { 0, 1, 2, 4, 5, 8, 15 };
    static const uint8_t ph_pins[] = { 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15 };
    static const uint8_t pi_pins[] = { 0, 1, 2, 3, 4, 5, 6, 7, 9, 10 };

    for (unsigned i = 0; i < sizeof(pd_pins); i++) fmc_pin(GPIOD_BASE, pd_pins[i]);
    for (unsigned i = 0; i < sizeof(pe_pins); i++) fmc_pin(GPIOE_BASE, pe_pins[i]);
    for (unsigned i = 0; i < sizeof(pf_pins); i++) fmc_pin(GPIOF_BASE, pf_pins[i]);
    for (unsigned i = 0; i < sizeof(pg_pins); i++) fmc_pin(GPIOG_BASE, pg_pins[i]);
    for (unsigned i = 0; i < sizeof(ph_pins); i++) fmc_pin(GPIOH_BASE, ph_pins[i]);
    for (unsigned i = 0; i < sizeof(pi_pins); i++) fmc_pin(GPIOI_BASE, pi_pins[i]);
}
