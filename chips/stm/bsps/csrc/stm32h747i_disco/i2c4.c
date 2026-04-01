/* C HAL binding for I2C4 on STM32H747I-DISCO.
 * Pins: PD12 (SCL, AF0, open-drain, pull-up), PD13 (SDA, AF0, open-drain, pull-up).
 * Equivalent to chips/stm/bsps/src/stm32h747i_disco/pac/i2c4.rs.
 */

#include "stm32h747xi.h"

void c_i2c4_enable_gpio_clocks(void) {
    /* Enable GPIOD clock (bit 3 in AHB4ENR) */
    REG32(RCC_AHB4ENR) |= (1u << 3);
}

void c_i2c4_configure_pins(void) {
    uint32_t tmp;

    /* PUPDR: pull-up (0b01) on PD12, PD13 */
    tmp = GPIOx(GPIOD_BASE, GPIO_PUPDR);
    tmp &= ~(0x3u << (12 * 2));
    tmp |=  (0x1u << (12 * 2));
    tmp &= ~(0x3u << (13 * 2));
    tmp |=  (0x1u << (13 * 2));
    GPIOx(GPIOD_BASE, GPIO_PUPDR) = tmp;

    /* OTYPER: open-drain (set bits) on PD12, PD13 */
    tmp = GPIOx(GPIOD_BASE, GPIO_OTYPER);
    tmp &= ~(1u << 12);
    tmp |=  (1u << 12);
    tmp &= ~(1u << 13);
    tmp |=  (1u << 13);
    GPIOx(GPIOD_BASE, GPIO_OTYPER) = tmp;

    /* OSPEEDR: low speed (0b00) on PD12, PD13 */
    tmp = GPIOx(GPIOD_BASE, GPIO_OSPEEDR);
    tmp &= ~(0x3u << (12 * 2));
    tmp &= ~(0x3u << (13 * 2));
    GPIOx(GPIOD_BASE, GPIO_OSPEEDR) = tmp;

    /* AFRL: no-op (matches Rust PAC behavior) */
    tmp = GPIOx(GPIOD_BASE, GPIO_AFRL);
    GPIOx(GPIOD_BASE, GPIO_AFRL) = tmp;

    /* AFRH: AF0 on PD12, PD13 */
    tmp = GPIOx(GPIOD_BASE, GPIO_AFRH);
    tmp &= ~(0xFu << ((12 % 8) * 4));
    tmp |=  (0u   << ((12 % 8) * 4));
    tmp &= ~(0xFu << ((13 % 8) * 4));
    tmp |=  (0u   << ((13 % 8) * 4));
    GPIOx(GPIOD_BASE, GPIO_AFRH) = tmp;

    /* MODER: alternate function (0b10) on PD12, PD13 */
    tmp = GPIOx(GPIOD_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (12 * 2));
    tmp |=  (0x2u << (12 * 2));
    tmp &= ~(0x3u << (13 * 2));
    tmp |=  (0x2u << (13 * 2));
    GPIOx(GPIOD_BASE, GPIO_MODER) = tmp;
}

void c_i2c4_enable_peripherals(void) {
    /* No peripheral clock enable needed (matches Rust PAC) */
}

void c_i2c4_deinit(void) {
    uint32_t tmp;

    /* Return PD12 to analog (0b11), clear pull, clear open-drain */
    tmp = GPIOx(GPIOD_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (12 * 2));
    tmp |=  (0x3u << (12 * 2));
    GPIOx(GPIOD_BASE, GPIO_MODER) = tmp;

    GPIOx(GPIOD_BASE, GPIO_PUPDR) &= ~(0x3u << (12 * 2));
    GPIOx(GPIOD_BASE, GPIO_OTYPER) &= ~(1u << 12);

    /* Return PD13 to analog */
    tmp = GPIOx(GPIOD_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (13 * 2));
    tmp |=  (0x3u << (13 * 2));
    GPIOx(GPIOD_BASE, GPIO_MODER) = tmp;

    GPIOx(GPIOD_BASE, GPIO_PUPDR) &= ~(0x3u << (13 * 2));
    GPIOx(GPIOD_BASE, GPIO_OTYPER) &= ~(1u << 13);

    /* Mask I2C4 interrupts */
    nvic_disable_irq(IRQ_I2C4_EV);
    nvic_disable_irq(IRQ_I2C4_ER);
}
